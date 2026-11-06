"""Optic Rift diffusers worker (protocol v1).

Runs in a dedicated virtual environment, started by the native host with `python -I`, offline
(HF_HUB_OFFLINE=1). It loads only safetensors weights from a local, hash-verified model folder,
never repository code, and speaks newline-delimited JSON:

  stdin  -> {"op": "health"} | {"op": "load", ...} | {"op": "generate", ...}
            | {"op": "cancel", "job_id": ...} | {"op": "unload"} | {"op": "shutdown"}
  stdout <- {"event": ..., ...}   (library chatter is redirected to stderr)
"""

import gc
import hashlib
import importlib.util
import json
import math
import os
import queue
import random
import sys
import threading
import time
import traceback

# python -I excludes adjacent modules from sys.path. Load only the exact
# app-owned helper staged by the native host, never a model-supplied module.
_REFERENCE_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "img2img_reference.py")
_REFERENCE_SPEC = importlib.util.spec_from_file_location("img2img_reference", _REFERENCE_PATH)
if _REFERENCE_SPEC is None or _REFERENCE_SPEC.loader is None:
    raise RuntimeError("Image reference verifier not installed")
img2img_reference = importlib.util.module_from_spec(_REFERENCE_SPEC)
_REFERENCE_SPEC.loader.exec_module(img2img_reference)

PROTOCOL = 1
# Library output must never corrupt the protocol stream.
_PROTOCOL_OUT = os.fdopen(os.dup(sys.stdout.fileno()), "w", encoding="utf-8", buffering=1)
sys.stdout = sys.stderr

ALLOWED_LIBRARIES = {"diffusers", "transformers"}
FORBIDDEN_SUFFIXES = (".py", ".pyc", ".bin", ".ckpt", ".pt", ".pth", ".pkl", ".pickle", ".msgpack", ".h5", ".onnx")
PIPELINES = {"diffusers-sdxl-text-to-image": "StableDiffusionXLPipeline"}
LIMITS = {"width": (256, 1536), "height": (256, 1536), "steps": (1, 100), "guidance": (0.0, 20.0)}
MAX_PROMPT_CHARS = 2000
NEWLINE = bytes([10])


def emit(event, **fields):
    fields["event"] = event
    _PROTOCOL_OUT.write(json.dumps(fields, separators=(",", ":")) + "\n")
    _PROTOCOL_OUT.flush()


class RequestError(Exception):
    """A request that is invalid; reported with category 'invalid_request'."""


def validate_model_dir(model_dir, adapter):
    """Static checks before any library touches the folder."""
    if adapter not in PIPELINES:
        raise RequestError(f"Unknown adapter {adapter!r}")
    index_path = os.path.join(model_dir, "model_index.json")
    with open(index_path, "r", encoding="utf-8") as fh:
        index = json.load(fh)
    if index.get("_class_name") != PIPELINES[adapter]:
        raise RequestError("model_index.json pipeline does not match the adapter")
    for name, value in index.items():
        if name.startswith("_") or not isinstance(value, list) or len(value) != 2:
            continue
        if value[0] is None and value[1] is None:
            continue
        if value[0] not in ALLOWED_LIBRARIES:
            raise RequestError(f"component {name} would load code from {value[0]!r}")
    for root, _dirs, files in os.walk(model_dir):
        for name in files:
            if name.lower().endswith(FORBIDDEN_SUFFIXES):
                raise RequestError(f"refusing model folder containing {name}")
    return index


def check_generate(req):
    prompt = req.get("prompt")
    if not isinstance(prompt, str) or not prompt.strip() or len(prompt) > MAX_PROMPT_CHARS:
        raise RequestError("prompt must be 1-2000 characters")
    negative = req.get("negative_prompt") or ""
    if not isinstance(negative, str) or len(negative) > MAX_PROMPT_CHARS:
        raise RequestError("negative prompt too long")
    out = {"prompt": prompt.strip(), "negative_prompt": negative.strip()}
    for key in ("width", "height", "steps"):
        value = req.get(key)
        low, high = LIMITS[key]
        if not isinstance(value, int) or isinstance(value, bool) or not low <= value <= high:
            raise RequestError(f"{key} must be an integer in {low}..{high}")
        if key in ("width", "height") and value % 8:
            raise RequestError(f"{key} must be a multiple of 8")
        out[key] = value
    guidance = req.get("guidance")
    low, high = LIMITS["guidance"]
    if not isinstance(guidance, (int, float)) or isinstance(guidance, bool) or not low <= guidance <= high or not math.isfinite(guidance):
        raise RequestError("guidance out of range")
    out["guidance"] = float(guidance)
    seed = req.get("seed")
    if seed is None:
        seed = random.SystemRandom().randrange(0, 2**32)
    if not isinstance(seed, int) or isinstance(seed, bool) or not 0 <= seed < 2**32:
        raise RequestError("seed must be an integer in 0..2^32-1")
    out["seed"] = seed
    output = req.get("output_path")
    if not isinstance(output, str) or not output.lower().endswith(".png") or not os.path.isabs(output):
        raise RequestError("output_path must be an absolute .png path")
    if os.path.exists(output):
        raise RequestError("output_path already exists")
    out["output_path"] = output
    return out


class Worker:
    def __init__(self):
        self.pipe = None
        self.image_pipe = None
        self.loaded = None
        self.cancel = threading.Event()
        self.current_job = None

    def torch(self):
        import torch  # noqa: PLC0415 - imported lazily so protocol errors are fast

        return torch

    def health(self, _req):
        torch = self.torch()
        info = {"protocol": PROTOCOL, "python": sys.version.split()[0], "torch": torch.__version__,
                "cuda_available": torch.cuda.is_available(), "cuda": torch.version.cuda}
        import diffusers  # noqa: PLC0415

        info["diffusers"] = diffusers.__version__
        if torch.cuda.is_available():
            free, total = torch.cuda.mem_get_info()
            info.update(device=torch.cuda.get_device_name(0),
                        capability=".".join(map(str, torch.cuda.get_device_capability(0))),
                        vram_free_mb=free // 2**20, vram_total_mb=total // 2**20)
        emit("health", **info)

    def load(self, req):
        model_dir, adapter = req.get("model_dir"), req.get("adapter")
        if not isinstance(model_dir, str) or not os.path.isabs(model_dir):
            raise RequestError("model_dir must be absolute")
        validate_model_dir(model_dir, adapter)
        if self.loaded == (model_dir, adapter):
            emit("loaded", cached=True, **self.memory())
            return
        self.unload(None, quiet=True)
        torch = self.torch()
        if not torch.cuda.is_available():
            raise RequestError("No CUDA GPU is available to this runtime")
        self.apply_budget(req)
        import diffusers  # noqa: PLC0415

        started = time.perf_counter()
        variant = "fp16" if req.get("variant") == "fp16" else None
        pipeline_cls = getattr(diffusers, PIPELINES[adapter])
        pipe = pipeline_cls.from_pretrained(model_dir, torch_dtype=torch.float16, variant=variant,
                                            use_safetensors=True, local_files_only=True)
        pipe.set_progress_bar_config(disable=True)
        pipe.to("cuda")
        self.pipe, self.loaded = pipe, (model_dir, adapter)
        emit("loaded", cached=False, seconds=round(time.perf_counter() - started, 2), **self.memory())

    def apply_budget(self, req):
        """Limit the PyTorch CUDA caching allocator, not all process or driver VRAM.

        Host-side GPU snapshots and this allocator guard reduce risk but cannot promise
        that every CUDA allocation or a competing game stays under a fixed bound.
        """
        budget = req.get("vram_budget_mb")
        if budget is None:
            raise RequestError("vram_budget_mb is required; the host must check GPU sharing first")
        torch = self.torch()
        _free, total = torch.cuda.mem_get_info()
        torch.cuda.set_per_process_memory_fraction(budget_fraction(budget, total // 2**20), 0)

    def memory(self):
        torch = self.torch()
        free, total = torch.cuda.mem_get_info()
        return {"vram_allocated_mb": torch.cuda.memory_allocated() // 2**20,
                "vram_free_mb": free // 2**20, "vram_total_mb": total // 2**20}

    def generate(self, req):
        if self.pipe is None:
            raise RequestError("No model is loaded")
        job_id = req.get("job_id")
        if not isinstance(job_id, str) or not job_id:
            raise RequestError("job_id required")
        params = check_generate(req)
        mode = req.get("mode", "text-to-image")
        if mode not in ("text-to-image", "image-to-image"):
            raise RequestError("Unsupported image generation mode")
        reference_image, strength = None, None
        if mode == "image-to-image":
            if self.loaded is None or self.loaded[1] != "diffusers-sdxl-text-to-image":
                raise RequestError("Image-to-image requires a locally installed SDXL text-to-image model")
            try:
                strength = img2img_reference.check_strength(req.get("strength"))
                reference_image = img2img_reference.prepare_reference(
                    req.get("reference"), params["width"], params["height"])
            except img2img_reference.ReferenceError as exc:
                raise RequestError(str(exc)) from exc
        self.apply_budget(req)
        torch = self.torch()
        self.cancel.clear()
        self.current_job = job_id
        steps = params["steps"]
        progress_total = max(1, int(steps * strength)) if strength is not None else steps
        torch.cuda.reset_peak_memory_stats()

        def on_step_end(pipe, step, _timestep, kwargs):
            emit("progress", job_id=job_id, step=step + 1, total=progress_total)
            if self.cancel.is_set():
                pipe._interrupt = True  # documented diffusers interruption flag
            return kwargs

        started = time.perf_counter()
        try:
            generator = torch.Generator(device="cuda").manual_seed(params["seed"])
            with torch.inference_mode():
                if mode == "image-to-image":
                    if self.image_pipe is None:
                        from diffusers import AutoPipelineForImage2Image  # noqa: PLC0415
                        # Shares underlying SDXL weights; does not load a second checkpoint.
                        self.image_pipe = AutoPipelineForImage2Image.from_pipe(self.pipe)
                        self.image_pipe.set_progress_bar_config(disable=True)
                    result = self.image_pipe(
                        prompt=params["prompt"], negative_prompt=params["negative_prompt"] or None,
                        image=reference_image, strength=strength, num_inference_steps=steps,
                        guidance_scale=params["guidance"], generator=generator,
                        callback_on_step_end=on_step_end)
                else:
                    result = self.pipe(prompt=params["prompt"], negative_prompt=params["negative_prompt"] or None,
                                       width=params["width"], height=params["height"], num_inference_steps=steps,
                                       guidance_scale=params["guidance"], generator=generator,
                                       callback_on_step_end=on_step_end)
        except torch.cuda.OutOfMemoryError:
            self.recover()
            emit("error", job_id=job_id, category="out_of_memory",
                 message="The GPU ran out of memory. Try a smaller size, or close other GPU-heavy apps.")
            return
        finally:
            self.current_job = None
        seconds = round(time.perf_counter() - started, 2)
        if self.cancel.is_set():
            self.recover()
            emit("cancelled", job_id=job_id, seconds=seconds)
            return
        image = result.images[0]
        problem = image_problem(image, params["width"], params["height"])
        if problem:
            emit("error", job_id=job_id, category="invalid_output", message=problem)
            return
        sha256 = save_png(image, params["output_path"])
        peak = torch.cuda.max_memory_allocated() // 2**20
        del result, image
        # Return cached working memory to the driver so other apps (games) get it back between jobs;
        # only the model weights stay resident until unload.
        self.recover()
        emit("done", job_id=job_id, path=params["output_path"], sha256=sha256, seed=params["seed"],
             width=params["width"], height=params["height"], seconds=seconds, peak_vram_mb=peak,
             resident_vram_mb=torch.cuda.memory_reserved() // 2**20)

    def recover(self):
        gc.collect()
        torch = self.torch()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()

    def unload(self, _req, quiet=False):
        had = self.pipe is not None
        self.pipe, self.image_pipe, self.loaded = None, None, None
        self.recover()
        if not quiet:
            info = self.memory() if self.torch().cuda.is_available() else {}
            emit("unloaded", had_model=had, **info)


def budget_fraction(budget_mb, total_mb):
    """Fraction allowed to PyTorch's caching allocator; not a system-wide VRAM cap."""
    if not isinstance(budget_mb, int) or isinstance(budget_mb, bool) or budget_mb <= 0:
        raise RequestError("vram_budget_mb must be a positive integer")
    if not isinstance(total_mb, int) or total_mb <= 0:
        raise RequestError("device memory size unknown")
    return min(1.0, budget_mb / total_mb)


def image_problem(image, width, height):
    """Reject blank, non-finite or wrongly sized output (e.g. fp16 VAE overflow)."""
    import numpy as np  # noqa: PLC0415

    if image.size != (width, height):
        return f"Output was {image.size[0]}x{image.size[1]}, expected {width}x{height}"
    pixels = np.asarray(image, dtype=np.float32)
    if not np.isfinite(pixels).all():
        return "Output contained invalid pixel values"
    if float(pixels.std()) < 2.0:
        return "Output was blank (a uniform image); generation failed numerically"
    return None


def save_png(image, path):
    partial = path + ".partial"
    image.save(partial, format="PNG")
    digest = hashlib.sha256()
    with open(partial, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            digest.update(chunk)
    os.replace(partial, path)
    return digest.hexdigest()


def stdin_lines():
    """Yield request lines without ever leaving a blocking read pending on Windows.

    A synchronous ReadFile pending on the stdin pipe makes other threads' GetFileType calls on
    that handle block, which deadlocked `import torch` in the main thread. On a pipe we poll
    with PeekNamedPipe and only read bytes that are already available.
    """
    fd = sys.stdin.fileno()
    if os.name == "nt":
        import ctypes  # noqa: PLC0415
        import msvcrt  # noqa: PLC0415
        from ctypes import wintypes  # noqa: PLC0415

        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel32.GetFileType.argtypes = [wintypes.HANDLE]
        kernel32.PeekNamedPipe.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p,
                                           ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
        handle = wintypes.HANDLE(msvcrt.get_osfhandle(fd))
        if kernel32.GetFileType(handle) == 3:  # FILE_TYPE_PIPE
            available = wintypes.DWORD()
            buffer = b""
            while True:
                if not kernel32.PeekNamedPipe(handle, None, 0, None, ctypes.byref(available), None):
                    break  # writer closed the pipe
                if not available.value:
                    time.sleep(0.02)
                    continue
                chunk = os.read(fd, available.value)
                if not chunk:
                    break
                buffer += chunk
                while NEWLINE in buffer:
                    line, buffer = buffer.split(NEWLINE, 1)
                    yield line.decode("utf-8", "replace")
            if buffer:
                yield buffer.decode("utf-8", "replace")
            return
    yield from sys.stdin


def main():
    worker = Worker()
    requests = queue.Queue()

    def reader():
        for line in stdin_lines():
            line = line.strip()
            if not line:
                continue
            try:
                req = json.loads(line)
            except json.JSONDecodeError:
                emit("error", category="invalid_request", message="request is not JSON")
                continue
            if isinstance(req, dict) and req.get("op") == "cancel":
                if req.get("job_id") == worker.current_job:
                    worker.cancel.set()
                continue
            requests.put(req)
        requests.put({"op": "shutdown"})

    threading.Thread(target=reader, daemon=True).start()
    emit("ready", protocol=PROTOCOL)
    handlers = {"health": worker.health, "load": worker.load, "generate": worker.generate, "unload": worker.unload}
    while True:
        req = requests.get()
        op = req.get("op") if isinstance(req, dict) else None
        if op == "shutdown":
            worker.unload(None, quiet=True)
            emit("shutdown")
            return
        handler = handlers.get(op)
        if handler is None:
            emit("error", category="invalid_request", message=f"unknown op {op!r}")
            continue
        try:
            handler(req)
        except RequestError as exc:
            emit("error", job_id=req.get("job_id"), op=op, category="invalid_request", message=str(exc))
        except Exception as exc:  # noqa: BLE001 - report and keep the worker alive
            traceback.print_exc(file=sys.stderr)
            worker.recover()
            emit("error", job_id=req.get("job_id"), op=op, category="worker_error",
                 message=f"{type(exc).__name__}: {str(exc)[:300]}")


if __name__ == "__main__":
    main()
