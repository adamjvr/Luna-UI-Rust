# M3.1a Retained Host Pipeline

M3.1a established a measurable CPU-host baseline before editor caching, gallery retention, or GPU
rendering changes the shape of the pipeline.

## Pipeline

```text
consume typed invalidations
    -> build immutable UiFrame
    -> render into retained BGRA8 Framebuffer
    -> resize softbuffer only if physical dimensions changed
    -> convert directly into acquired 0x00RRGGBB surface storage
    -> present
    -> translate the current accessibility tree
    -> record and periodically report stage timings
```

## Retained resources

The native host owns one CPU framebuffer. It recreates the allocation only when the physical window
size changes. Before each render it clears the retained storage to transparent black, preserving the
same initial state as a freshly allocated framebuffer.

Softbuffer surface configuration is tracked separately and repeated only after an actual physical
size change or native-surface recreation. BGRA8 pixels convert directly into softbuffer's mapped
`u32` storage, avoiding an intermediate full-frame vector and second copy.

## Invalidation classes

M3.1a introduced stable categories for initial frames, animation, paint overlays, text overlays, text
raster, text layout, widget layout, accessibility, surface exposure, complete frames, and explicit
application work. `HostControl::Invalidate` lets applications select a precise class while the host
retains source-specific defaults for existing redraw requests.

## Diagnostics

The host reports application frame construction, CPU rendering, packed-pixel conversion,
presentation, accessibility translation, and total frame time. Lifetime counters track framebuffer
allocations, surface resizes, accessibility translations, and invalidation classes.

M3.1b consumes these invalidation classes for editor text and chrome. M3.1c uses them to skip
unchanged gallery and accessibility work.
