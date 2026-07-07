## 6. Result and waveform types

```
OpResult
  v(a: Net, b: Net = gnd) -> Real
  i(a: Net, b: Net = gnd) -> Real

Trace                                       // $tran and $ac
  v(a: Net, b: Net = gnd) -> Waveform<T>    // T = Real ($tran) | Complex ($ac)
  i(a: Net, b: Net = gnd) -> Waveform<T>
  axis() -> Waveform<Real>                  // time or frequency

NoiseTrace
  psd()   -> Waveform<Real>       // output-referred PSD, V²/Hz
  total() -> Real                 // integrated RMS noise over the sweep band, V

Waveform<T>                                 // a generic series over the analysis axis
  at(x: Real) -> T
  points() -> Vec<(Real, T)>
  len() -> Natural
  map(f: fn(T) -> U) -> Waveform<U>         // arbitrary transforms
  // T = Real:
  min() / max() / mean() / rms() / peak_to_peak() -> Real
  cross(level: Real, dir: CrossDir = Either) -> Option<Real>
  rise_time(lo: Real, hi: Real) -> Option<Real>   ;  fall_time(...) -> Option<Real>
  fft() -> Waveform<Complex>
  // T = Complex:
  mag() / phase() / db() -> Waveform<Real>
```

`Waveform<T>` is a generic value-layer type (Part I §6.6). It is the point of the design: signal
post-processing is **library** work over `Waveform`, not built-in tasks. Beyond the methods
above, FFT-derived measures (THD, SNR, spectral peaks), eye diagrams, and windowing are library
functions over `points()`/`fft()`, added without touching the language.

---

