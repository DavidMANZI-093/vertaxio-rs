# vertaxio-rs

> *Aim as good as Masterchief, bruv.*

[Watch demo clip (~11 min)](https://archive.org/details/vertaxio-demo-clip)

A real-time game-vision research tool for **Call of Duty 4: Modern Warfare**, rewritten from scratch in Rust — purpose-driven for maximum performance on modest hardware, with zero dependency on OpenCV.

## Background & Motivation

This project has two lives.

**Version 1 — `CoD4-MW_AimOptimizer` (C++/OpenCV, ~2 years ago)**
The original was built quickly with heavy AI assistance and leaned on battle-tested but heavyweight libraries: **OpenCV** for all image processing, **DirectX 11** via raw C-style DXGI globals, and a single-threaded game loop that mixed capture, detection, debug rendering, and input polling all in one. It worked, but it carried every byte of OpenCV's runtime, used unbounded allocations per frame, and had no concept of structured concurrency or graceful resource cleanup. CPU and RAM usage were high because the tool never questioned *what it actually needed.*

**Version 2 — `vertaxio-rs` (this repo)**
A ground-up, engineering-first rewrite in Rust. No OpenCV. No framework crutches. Every subsystem was designed around a single question: *what is the minimum correct work required to achieve this at 60 fps on a 2.5–3.1 GHz CPU with UHD 620 integrated graphics and 8 GB RAM?*

The answer fits in **~26–30 MB of RAM** and runs at **40–50% CPU** and just **2–3% GPU** — including full debug window rendering — even when profiled on an old 3.2 GHz system with no GPU and only 6 GB RAM.

## Architecture Overview

```
main.rs  (input/control loop — main thread)
│
├── services/
│   ├── parser.rs    — YAML config loader & validator
│   ├── input.rs     — Win32 GetAsyncKeyState wrapper
│   └── errors.rs    — unified XError enum (thiserror)
│
└── core/            (runs inside a dedicated CV thread)
    ├── capture.rs   — DXGI Desktop Duplication capture
    ├── vision.rs    — HSV thresholding + morphology + contours (Rayon-parallel)
    ├── debug.rs     — minifb debug window (zero-copy pixel write)
    └── monitor.rs   — multi-monitor enumeration & interactive selection
```

Two threads, cleanly separated by concern:

| Thread | Responsibility |
|---|---|
| **Main** | Key polling at 10 ms intervals, Day/Night profile switching, graceful shutdown signalling |
| **CV Worker** | Frame capture → HSV threshold → dilation → contour extraction → debug render, frame-rate capped |

Shared state between threads is strictly two `Arc<AtomicBool>` values — `is_active` and `should_stop`. No mutexes, no channels, no lock contention.

## Performance Engineering Decisions

### 1. Eliminating OpenCV Entirely

The original used OpenCV for: color-space conversion, `inRange` thresholding, morphological operations, contour finding, bounding box computation, and debug window rendering. OpenCV is a large, dynamically-linked runtime (~20–80 MB depending on build) that solves a far broader problem space than this tool needs.

**vertaxio-rs replaces every one of those operations with purpose-built Rust:**

| Task | Old (C++/OpenCV) | New (Rust) |
|---|---|---|
| HSV conversion | `cv::cvtColor` | Inline `rgb_to_hsv()` — integer-only, `#[inline(always)]` |
| Color thresholding | `cv::inRange` | Rayon parallel iterator over BGRA chunks |
| Dilation/morphology | `cv::morphologyEx` | Custom 3×3 parallel row dilation |
| Contour finding | `cv::findContours` | `imageproc::contours::find_contours` |
| Debug window | `cv::imshow` / `cv::waitKey` | `minifb` — direct framebuffer write |

The result: no OpenCV runtime loaded, no runtime JNI/FFI bridge, no `cv::Mat` heap allocations per frame.

### 2. Custom Integer-Only HSV Conversion

```rust
#[inline(always)]
fn rgb_to_hsv(r_u8: u8, g_u8: u8, b_u8: u8) -> (i32, i32, i32) {
    // all arithmetic stays in i32 — no floats, no divisions beyond one each for s and h
}
```

OpenCV's `cvtColor` works on floating-point internally and processes the full frame as a contiguous matrix allocation. This implementation:

- Uses **integer arithmetic only** — no floats, no transcendental functions.
- Is **inlined at every call site** — zero function-call overhead.
- Lives inside a Rayon parallel iterator so all cores process chunks simultaneously.
- Never allocates: it's called per-pixel inside a `par_iter_mut().zip(par_chunks_exact(4))` — no intermediate `Mat` buffers.

The **hue wrap-around** case (`lower_bound[0] > upper_bound[0]`) is handled explicitly, matching OpenCV's `inRange` semantics for red (which straddles the 0°/360° boundary) without a second mask pass.

### 3. Rayon Parallel Thresholding — One Allocation, Full Parallelism

```rust
let mut mask_buffer = vec![0u8; area]; // single allocation: W × H bytes

mask_buffer.par_iter_mut()
    .zip(bgra_buffer.par_chunks_exact(4))
    .for_each(|(mask_pixel, bgra)| {
        // per-pixel HSV test, written directly into mask_buffer
    });
```

- **One allocation** for the entire mask — `vec![0u8; W*H]`.
- Rayon distributes work across all available CPU cores automatically.
- `par_chunks_exact(4)` eliminates any per-pixel bounds checking for the 4-byte BGRA stride.
- No intermediate owned objects, no frame copying — the same `Vec<u8>` frame buffer from the DXGI capture is passed directly as a slice.

This is the single hottest path in the entire application and it has been designed so that the only work done per pixel is: 5 array reads, 9 integer arithmetic operations, 3 comparisons, and one conditional write.

### 4. Row-Parallel Dilation (No Cross-Thread Aliasing)

Morphological dilation expands detected pixels to bridge gaps caused by loose saturation constraints. The naive serial approach iterates over every pixel twice. This implementation:

```rust
dilated_mask.par_chunks_exact_mut(w)
    .enumerate()
    .for_each(|(y, row_slice)| {
        // inspect 3×3 neighborhood from mask_buffer (read-only)
        // write only into row_slice (exclusive mutable slice of dilated_mask)
    });
```

- Each thread owns its output row exclusively — **no atomic writes, no mutexes**.
- Reads from `mask_buffer` are immutable shared borrows — safe across all threads simultaneously.
- Uses `saturating_sub` and `.min()` for boundary clamping — no branch on every edge pixel.
- The `'outer` labeled break exits both inner loops immediately on first match — no wasted work once the 3×3 neighborhood passes.

### 5. DXGI Frame Capture: Staging Texture + Manual Row Pitch Copy

```rust
// D3D11_USAGE_STAGING texture — only a CPU-accessible copy, not a GPU render target
let texture_desc = D3D11_TEXTURE2D_DESC {
    Usage: D3D11_USAGE_STAGING,
    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
    ...
};
```

The DXGI Desktop Duplication API delivers frames as GPU textures. To read them on the CPU they must be mapped. The critical engineering detail is **row pitch**: GPU memory is row-padded to alignment boundaries, so `mapped.RowPitch` is often larger than `width * 4`. The capture loop copies only the valid `width_bytes` per row, discarding padding — resulting in a tightly-packed `Vec<u8>` with no wasted bytes passed downstream.

```rust
for y in 0..height {
    let src_row = src_ptr.add(y * row_pitch);     // GPU pitch (padded)
    let dst_row = dst_ptr.add(y * width_bytes);   // CPU buffer (tight)
    copy_nonoverlapping(src_row, dst_row, width_bytes);
}
```

The staging texture is recreated each frame (a necessary DXGI constraint), but the output `Vec<u8>` is sized exactly once and reused via `grab_frame`'s return — kept on the heap, passed by mutable reference through the pipeline to avoid double-allocation.

### 6. Debug Window: minifb + In-Place BGRA→0RGB Bitshift

OpenCV's `imshow` is a synchronous call that creates/manages its own window event loop, requires a `cv::Mat`, and triggers internal memory operations on every frame. `minifb` writes directly to a `Vec<u32>` framebuffer:

```rust
// BGRA[B, G, R, A] → 0RGB packed u32
self.draw_buffer[i] = (r << 16) | (g << 8) | b;
```

- No pixel format conversion function call — one bitshift expression per pixel.
- `draw_buffer` is allocated once at window creation (`Vec<u32>` of size W×H) and reused every frame.
- `minifb`'s `update_with_buffer` pushes the buffer directly to the window's GDI/OS surface — no intermediate copy.
- `window.set_target_fps(fps)` delegates frame-pacing to the OS, removing the need for manual sleep logic in the debug path.
- When debug mode is disabled (`enabled = false`), the entire `draw()` function returns on the first line — zero work done.

Bounding boxes are drawn via direct index arithmetic into `draw_buffer` — no drawing library, no path rasterization, just writes at `y * w + x`.

### 7. Two-Thread Architecture: Decoupled Input from CV Work

```
Main thread:  polls keys every 10 ms → sets is_active / should_stop atomics
CV thread:    captures frames → processes → renders → sleeps to hit target FPS
```

This decoupling means:

- **Key polling is never delayed by a slow frame.** A frame taking 20 ms at 60 fps never causes a missed key press.
- **The CV thread goes fully idle** when `is_active` is false — it calls `debug_window.update()` (OS event pump only) and sleeps for a full frame period. CPU drops to near-zero during standby.
- **Shutdown is cooperative and clean.** `should_stop` causes the CV thread to exit its loop naturally, then `cv_thread.join()` in main waits for it — no forced termination, no leaked GPU resources.

### 8. Structured Error Handling: `thiserror` + `XError`

```rust
#[derive(Error, Debug)]
pub enum XError {
    ConfigError(String),
    SystemError(String),
    Timeout,        // ← non-error DXGI_ERROR_WAIT_TIMEOUT is its own variant
    VisionError(String),
    IoError(#[from] std::io::Error),
}
```

`Timeout` is a first-class variant rather than a string match. When `AcquireNextFrame` returns `DXGI_ERROR_WAIT_TIMEOUT` (a normal case — no new frame yet), the capture returns `Err(XError::Timeout)` and the match arm simply ticks the debug window. No log spam, no panic, no exception overhead.

### 9. Configuration: YAML + Compile-Time Validated Defaults

`lamine.yml` is loaded at startup via `serde_yaml`. The `RawConfig` struct holds raw strings and arrays; `Config::validate()` converts them to typed Windows `VIRTUAL_KEY` values and checked ranges. Invalid configs fail fast at startup with a clear message — not at runtime during a frame.

Sensible defaults are declared via `#[serde(default = "fn_name")]` per-field — unknown keys in the YAML are silently ignored. Adding new tuning parameters requires touching exactly one struct and one default function, not scattered constants.

### 10. Monitor Enumeration: Interactive, Not Hardcoded

The old version hardcoded `screenWidth = 1366, screenHeight = 768` and a derived capture region. `vertaxio-rs` calls `EnumDisplayMonitors` at startup, collects all connected displays with their real resolutions, and if more than one is found prompts the user to select with a single keypress (via `getch` — raw terminal, no Enter required). The selected monitor's `HMONITOR` handle is passed directly to DXGI, so capture always matches the physical resolution of the chosen display — no magic numbers, no manual reconfiguration for different setups.

## Key Bindings & Runtime

| Key | Action |
|---|---|
| `K` (hold) | Activate processing |
| `M` | Toggle Day / Night HSV filter profile |
| `Q` | Exit cleanly |

All keys are configurable in `lamine.yml`.

## Configuration (`lamine.yml`)

```yaml
exit_key: "Q"
trigger_key: "K"
mode_switch_key: "M"
fps: 60
debug_mode: true

# Day profile (higher saturation threshold, brighter conditions)
day_hsv_low:  [0, 142, 98]
day_hsv_high: [10, 255, 255]

# Night profile (relaxed saturation for darker scenes)
night_hsv_low:  [0, 122, 78]
night_hsv_high: [12, 255, 255]
```

HSV values follow OpenCV's 0-180 hue convention. Adjust the `[H, S, V]` bounds to match the enemy marker color in your game's map/lighting conditions. Switch between profiles at runtime with `M`.

## Dependencies

| Crate | Role |
|---|---|
| `windows` | Win32 API bindings (DXGI, D3D11, keyboard input, console) |
| `rayon` | Data-parallel iterators for the vision pipeline |
| `image` + `imageproc` | Image buffer wrapper + contour finding |
| `minifb` | Lightweight native framebuffer window |
| `serde` + `serde_yaml` | Config deserialization |
| `argh` | CLI argument parsing |
| `getch` | Raw single-keypress input for monitor selection |
| `thiserror` | Ergonomic error type derivation |

No OpenCV. No runtime C++ ABI. No dynamic linking to large native libraries.

## Build

```bash
cargo build --release
```

Requires:
- Rust toolchain (edition 2024)
- Windows target (`x86_64-pc-windows-msvc`)
- Windows SDK (for DXGI/D3D11 headers, provided by `windows` crate)

Run from the project root so `lamine.yml` is resolved at the default path, or pass `-c path/to/config.yml`.

## Disclaimer

This tool is strictly for **research and educational purposes** — specifically, understanding real-time computer vision pipelines, DXGI Desktop Duplication, and performance-constrained systems programming in Rust. It is not intended to be used in live gameplay to gain an unfair advantage. The author does not endorse cheating in video games.
