"""Protocol and safety tests for the diffusers worker that need no GPU or model.

Run: python -I -m unittest discover -s src-tauri/workers -p "test_*.py"
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import diffusers_worker as w  # noqa: E402

SDXL_INDEX = {
    "_class_name": "StableDiffusionXLPipeline",
    "scheduler": ["diffusers", "EulerDiscreteScheduler"],
    "text_encoder": ["transformers", "CLIPTextModel"],
    "unet": ["diffusers", "UNet2DConditionModel"],
    "feature_extractor": [None, None],
}


def model_dir(index=SDXL_INDEX, extra=()):
    root = tempfile.mkdtemp()
    with open(os.path.join(root, "model_index.json"), "w", encoding="utf-8") as fh:
        json.dump(index, fh)
    for name in extra:
        path = os.path.join(root, name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        open(path, "w").close()
    return root


class ValidateModelDir(unittest.TestCase):
    def test_accepts_standard_sdxl_folder(self):
        self.assertEqual(w.validate_model_dir(model_dir(), "diffusers-sdxl-text-to-image")["_class_name"],
                         "StableDiffusionXLPipeline")

    def test_rejects_unknown_adapter_and_wrong_pipeline(self):
        with self.assertRaises(w.RequestError):
            w.validate_model_dir(model_dir(), "anything")
        with self.assertRaises(w.RequestError):
            w.validate_model_dir(model_dir({**SDXL_INDEX, "_class_name": "FluxPipeline"}), "diffusers-sdxl-text-to-image")

    def test_rejects_repository_code_and_pickles(self):
        with self.assertRaises(w.RequestError):
            w.validate_model_dir(model_dir({**SDXL_INDEX, "unet": ["evil_module", "UNet"]}), "diffusers-sdxl-text-to-image")
        for bad in ("unet/model.bin", "scheduler/custom.py", "x.ckpt", "vae/weights.pt"):
            with self.subTest(bad=bad), self.assertRaises(w.RequestError):
                w.validate_model_dir(model_dir(extra=[bad]), "diffusers-sdxl-text-to-image")


class CheckGenerate(unittest.TestCase):
    def base(self, **over):
        req = {"prompt": "a lighthouse", "width": 1024, "height": 1024, "steps": 20, "guidance": 7.0,
               "seed": 42, "output_path": os.path.join(tempfile.mkdtemp(), "out.png")}
        req.update(over)
        return req

    def test_valid_request_is_normalised(self):
        out = w.check_generate(self.base(prompt="  a lighthouse  "))
        self.assertEqual(out["prompt"], "a lighthouse")
        self.assertEqual(out["seed"], 42)

    def test_missing_seed_is_drawn_and_reported(self):
        out = w.check_generate(self.base(seed=None))
        self.assertTrue(0 <= out["seed"] < 2**32)

    def test_rejections(self):
        cases = [dict(prompt=""), dict(prompt="x" * 2001), dict(width=1000 + 3), dict(width=4096), dict(height=128),
                 dict(steps=0), dict(steps=True), dict(guidance=float("nan")), dict(guidance=50), dict(seed=-1),
                 dict(seed=2**32), dict(output_path="relative.png"), dict(output_path="C:/x/out.jpg")]
        for case in cases:
            with self.subTest(case=case), self.assertRaises(w.RequestError):
                w.check_generate(self.base(**case))

    def test_refuses_to_overwrite(self):
        path = os.path.join(tempfile.mkdtemp(), "exists.png")
        open(path, "w").close()
        with self.assertRaises(w.RequestError):
            w.check_generate(self.base(output_path=path))


class Budget(unittest.TestCase):
    def test_fraction_is_bounded_and_validated(self):
        self.assertAlmostEqual(w.budget_fraction(8000, 16000), 0.5)
        self.assertEqual(w.budget_fraction(20000, 16000), 1.0)
        for bad in (0, -5, None, True, 1.5):
            with self.subTest(bad=bad), self.assertRaises(w.RequestError):
                w.budget_fraction(bad, 16000)

    def test_generate_and_load_require_a_budget_from_the_host(self):
        worker = w.Worker()
        worker.pipe = object()
        req = {"job_id": "j", "prompt": "x", "width": 512, "height": 512, "steps": 2, "guidance": 1.0, "seed": 1,
               "output_path": os.path.join(tempfile.mkdtemp(), "o.png")}
        with self.assertRaises(w.RequestError):
            worker.apply_budget(req)


class ProtocolProcess(unittest.TestCase):
    """Run the real worker process and talk to it over stdin/stdout without torch work."""

    def run_worker(self, lines):
        proc = subprocess.run([sys.executable, "-I", os.path.join(HERE, "diffusers_worker.py")],
                              input="\n".join(lines) + "\n", capture_output=True, text=True, timeout=60)
        return [json.loads(line) for line in proc.stdout.splitlines()]

    def test_protocol_stream_is_pure_json_and_errors_keep_worker_alive(self):
        events = self.run_worker(["not json", json.dumps({"op": "nope"}),
                                  json.dumps({"op": "generate", "job_id": "j1"}), json.dumps({"op": "shutdown"})])
        self.assertEqual(events[0], {"event": "ready", "protocol": 1})
        kinds = [(e["event"], e.get("category")) for e in events[1:]]
        self.assertEqual(kinds, [("error", "invalid_request"), ("error", "invalid_request"),
                                 ("error", "invalid_request"), ("shutdown", None)])

    def test_load_rejects_bad_folder_before_importing_models(self):
        bad = model_dir(extra=["unet/x.bin"])
        events = self.run_worker([json.dumps({"op": "load", "model_dir": bad, "adapter": "diffusers-sdxl-text-to-image"}),
                                  json.dumps({"op": "shutdown"})])
        self.assertEqual(events[1]["category"], "invalid_request")
        self.assertIn("x.bin", events[1]["message"])


class OpenStdinHealth(unittest.TestCase):
    """Regression: with stdin held open (as the desktop host does), importing torch must not hang."""

    def test_health_answers_while_stdin_stays_open(self):
        try:
            import torch  # noqa: F401, PLC0415
        except ImportError:
            self.skipTest("torch is not installed in this interpreter")
        import threading  # noqa: PLC0415

        proc = subprocess.Popen([sys.executable, "-I", os.path.join(HERE, "diffusers_worker.py")], stdin=subprocess.PIPE,
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        try:
            self.assertEqual(json.loads(proc.stdout.readline())["event"], "ready")
            proc.stdin.write(json.dumps({"op": "health"}) + chr(10))
            proc.stdin.flush()
            box = {}
            reader = threading.Thread(target=lambda: box.update(line=proc.stdout.readline()), daemon=True)
            reader.start()
            reader.join(180)
            self.assertIn("line", box, "worker hung while stdin was open")
            self.assertEqual(json.loads(box["line"])["event"], "health")
        finally:
            proc.kill()



class ImageToImageProtocol(unittest.TestCase):
    """Validate new generation mode without touching CUDA or model weights."""

    @staticmethod
    def base_request(**overrides):
        req = {
            "job_id": "job-cpu-only", "prompt": "a blue garden",
            "negative_prompt": "", "width": 512, "height": 512,
            "steps": 12, "guidance": 7.0, "seed": 8,
            "output_path": os.path.join(tempfile.mkdtemp(), "new.png"),
            "mode": "image-to-image", "strength": 0.65,
        }
        req.update(overrides)
        return req

    @staticmethod
    def loaded_worker():
        worker = w.Worker()
        worker.pipe = object()
        worker.loaded = ("C:/approved-local-model", "diffusers-sdxl-text-to-image")
        return worker

    def test_bad_image_to_image_input_fails_before_gpu_budget(self):
        from unittest import mock
        worker = self.loaded_worker()
        with mock.patch.object(worker, "apply_budget", side_effect=AssertionError("GPU invoked")):
            for invalid in (
                self.base_request(reference=None),
                self.base_request(reference={"job_id": "../escape"}),
                self.base_request(strength=0.0),
                self.base_request(strength=float("nan")),
                self.base_request(mode="video-to-video"),
            ):
                with self.subTest(request=invalid), self.assertRaises(w.RequestError):
                    worker.generate(invalid)

    def test_verified_reference_reaches_budget_boundary_without_cuda(self):
        from unittest import mock
        import hashlib
        import uuid
        from PIL import Image

        worker = self.loaded_worker()
        with tempfile.TemporaryDirectory() as folder:
            outputs = os.path.join(folder, "outputs")
            os.makedirs(outputs)
            job = str(uuid.uuid4())
            path = os.path.join(outputs, job + ".png")
            Image.new("RGB", (512, 512), (80, 140, 200)).save(path)
            with open(path, "rb") as handle:
                sha = hashlib.sha256(handle.read()).hexdigest()
            req = self.base_request(reference={"job_id": job, "path": path, "sha256": sha})
            with mock.patch.object(worker, "apply_budget", side_effect=RuntimeError("budget boundary reached")):
                with self.assertRaisesRegex(RuntimeError, "budget boundary reached"):
                    worker.generate(req)


if __name__ == "__main__":
    unittest.main()
