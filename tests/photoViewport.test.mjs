import assert from 'node:assert/strict';
import test from 'node:test';
import { photoViewportFit, photoPanBounds, photoClampPan, photoZoomStep } from '../src/photoViewport.ts';

test('fit preserves aspect and does not enlarge the 1400px preview proxy', () => {
  assert.deepEqual(photoViewportFit({width:1400,height:700},{width:700,height:400}),{width:700,height:350});
  assert.deepEqual(photoViewportFit({width:200,height:100},{width:1000,height:1000}),{width:200,height:100});
  assert.deepEqual(photoViewportFit({width:700,height:1400},{width:500,height:750}),{width:375,height:750});
});
test('invalid dimensions produce zero fit and never NaN transforms', () => {
  for (const i of [-1, 0, Infinity, NaN]) assert.deepEqual(photoViewportFit({width:i,height:100},{width:500,height:500}),{width:0,height:0});
  assert.deepEqual(photoPanBounds({width:0,height:1},{width:200,height:200},2),{x:0,y:0});
});
test('fit has no pan and zoom only pans on overflow axes', () => {
  const image={width:1400,height:700}, viewport={width:700,height:400};
  assert.deepEqual(photoPanBounds(image,viewport,1),{x:0,y:0});
  assert.deepEqual(photoPanBounds(image,viewport,2),{x:350,y:150});
  assert.deepEqual(photoPanBounds({width:100,height:100},{width:900,height:900},3),{x:0,y:0});
});
test('drag and keyboard offsets clamp on both axes', () => {
  const image={width:1400,height:700}, viewport={width:700,height:400};
  assert.deepEqual(photoClampPan({x:999,y:-999},image,viewport,2),{x:350,y:-150});
  assert.deepEqual(photoClampPan({x:40,y:50},image,viewport,1),{x:0,y:0});
  assert.deepEqual(photoClampPan({x:Infinity,y:NaN},image,viewport,3),{x:0,y:0});
});
test('zoom steps have fixed bounded values', () => {
  assert.equal(photoZoomStep(1,1),1.25);
  assert.equal(photoZoomStep(1,-1),1);
  assert.equal(photoZoomStep(1.5,1),2);
  assert.equal(photoZoomStep(4,1),4);
  assert.equal(photoZoomStep(2,-1),1.5);
  assert.equal(photoZoomStep(NaN,1),1);
});
test('view component uses one raster renderer; all inspection transforms remain CSS only', async () => {
  const {readFileSync}=await import('node:fs');
  const source=readFileSync(new URL('../src/PhotoCanvasPreview.tsx',import.meta.url),'utf8');
  assert.match(source, /paintAdjustedPhoto\(decoded\.bitmap, \{ brightness, contrast, rotation, crop \}, canvas, 1400\)/);
  assert.match(source, /transform: `translate\(\$\{actualPan\.x\}px, \$\{actualPan\.y\}px\) scale\(\$\{zoom\}\)`/);
  assert.match(source, /setPointerCapture\(event\.pointerId\)/);
  assert.match(source, /releasePointerCapture\(event\.pointerId\)/);
  assert.match(source, /new ResizeObserver\(update\)/);
  assert.match(source, /setZoom\(1\)/);
  assert.match(source, /event\.target !== event\.currentTarget/);
  assert.doesNotMatch(source, /canvas\.toBlob|localStorage|sessionStorage|invoke\(/);
});
