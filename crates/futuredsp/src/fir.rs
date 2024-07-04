//! FIR filters
use core::cmp::Ordering;
#[cfg(not(RUSTC_IS_STABLE))]
use core::intrinsics::{fadd_fast, fmul_fast};
use num_complex::Complex;
use num_traits::{Float, Zero};

use crate::ComputationStatus;
use crate::FirKernel;
use crate::Taps;

/// A non-resampling FIR filter. Calling `filter()` on this struct always
/// produces exactly as many samples as it consumes.
///
/// Implementations of this core exist for the following combinations:
/// - `f32` samples, `f32` taps.
/// - `Complex<f32>` samples, `f32` taps.
///
/// Example usage:
/// ```
/// use futuredsp::FirFilter;
/// use futuredsp::FirKernel;
///
/// let fir = FirFilter::<f32, f32, _, _>::new([1f32, 2f32, 3f32]);
///
/// let input = [1.0, 2.0, 3.0];
/// let mut output = [0.0];
/// fir.filter(&input, &mut output);
/// ```
pub struct FirFilter<InputType, OutputType, TA, TT>
where
    TA: Taps<TapType = TT>,
{
    taps: TA,
    _input_type: core::marker::PhantomData<InputType>,
    _output_type: core::marker::PhantomData<OutputType>,
}

impl<InputType, OutputType, TA, TT> FirFilter<InputType, OutputType, TA, TT>
where
    TA: Taps<TapType = TT>,
{
    /// Create a new non-resampling FIR filter using the given taps.
    pub fn new(taps: TA) -> Self {
        Self {
            taps,
            _input_type: core::marker::PhantomData,
            _output_type: core::marker::PhantomData,
        }
    }
}

/// Internal helper function to abstract away everything but the core computation.
/// Note that this function gets heavily inlined, so there is no (runtime) performance
/// overhead.
fn fir_kernel_core<
    InputType,
    OutputType,
    TapsType: Taps,
    InitFn: Fn() -> OutputType,
    MacFn: Fn(OutputType, InputType, TapsType::TapType) -> OutputType,
>(
    taps: &TapsType,
    i: &[InputType],
    o: &mut [OutputType],
    init: InitFn,
    mac: MacFn,
) -> (usize, usize, ComputationStatus)
where
    InputType: Copy,
    OutputType: Copy,
    TapsType::TapType: Copy,
{
    let num_producable_samples = (i.len() + 1).saturating_sub(taps.num_taps());
    let (n, status) = match num_producable_samples.cmp(&o.len()) {
        Ordering::Greater => (o.len(), ComputationStatus::InsufficientOutput),
        Ordering::Equal => (num_producable_samples, ComputationStatus::BothSufficient),
        Ordering::Less => (num_producable_samples, ComputationStatus::InsufficientInput),
    };

    unsafe {
        for k in 0..n {
            let mut sum = init();
            for t in 0..taps.num_taps() {
                sum = mac(
                    sum,
                    *i.get_unchecked(k + t),
                    taps.get(taps.num_taps() - 1 - t),
                );
            }
            *o.get_unchecked_mut(k) = sum;
        }
    }

    (n, n, status)
}

#[cfg(not(RUSTC_IS_STABLE))]
impl<TA: Taps<TapType = f32>> FirKernel<f32, f32> for FirFilter<f32, f32, TA, f32> {
    fn filter(&self, i: &[f32], o: &mut [f32]) -> (usize, usize, ComputationStatus) {
        fir_kernel_core(
            &self.taps,
            i,
            o,
            || 0.0,
            |accum, sample, tap| unsafe { fadd_fast(accum, fmul_fast(sample, tap)) },
        )
    }
}

#[cfg(RUSTC_IS_STABLE)]
impl<TA: Taps<TapType = f32>> UnaryKernel<f32, f32> for NonResamplingFirKernel<f32, f32, TA, f32> {
    fn filter(&self, i: &[f32], o: &mut [f32]) -> (usize, usize, ComputationStatus) {
        fir_kernel_core(
            &self.taps,
            i,
            o,
            || 0.0,
            |accum, sample, tap| accum + sample * tap,
        )
    }
}

#[cfg(not(RUSTC_IS_STABLE))]
impl<TA: Taps<TapType = f64>> FirKernel<f64, f64> for FirFilter<f64, f64, TA, f64> {
    fn filter(&self, i: &[f64], o: &mut [f64]) -> (usize, usize, ComputationStatus) {
        fir_kernel_core(
            &self.taps,
            i,
            o,
            || 0.0,
            |accum, sample, tap| unsafe { fadd_fast(accum, fmul_fast(sample, tap)) },
        )
    }
}

#[cfg(RUSTC_IS_STABLE)]
impl<TA: Taps<TapType = f64>> UnaryKernel<f64, f64> for NonResamplingFirKernel<f64, f64, TA, f64> {
    fn filter(&self, i: &[f64], o: &mut [f64]) -> (usize, usize, ComputationStatus) {
        fir_kernel_core(
            &self.taps,
            i,
            o,
            || 0.0,
            |accum, sample, tap| accum + sample * tap,
        )
    }
}

#[cfg(not(RUSTC_IS_STABLE))]
impl<TA: Taps<TapType = T>, T> FirKernel<Complex<T>, Complex<T>>
    for FirFilter<Complex<T>, Complex<T>, TA, T>
where
    T: Float + Send + Sync + Copy + Zero,
{
    fn filter(&self, i: &[Complex<T>], o: &mut [Complex<T>]) -> (usize, usize, ComputationStatus) {
        fir_kernel_core(
            &self.taps,
            i,
            o,
            || Complex {
                im: T::zero(),
                re: T::zero(),
            },
            |accum, sample, tap| Complex {
                re: unsafe { fadd_fast(accum.re, fmul_fast(sample.re, tap)) },
                im: unsafe { fadd_fast(accum.im, fmul_fast(sample.im, tap)) },
            },
        )
    }
}

#[cfg(RUSTC_IS_STABLE)]
impl<TA: Taps<TapType = T>, T> UnaryKernel<Complex<T>, Complex<T>>
    for NonResamplingFirKernel<Complex<T>, Complex<T>, TA, T>
where
    T: Float + Send + Sync + Copy + Zero,
{
    fn filter(&self, i: &[Complex<T>], o: &mut [Complex<T>]) -> (usize, usize, ComputationStatus) {
        fir_kernel_core(
            &self.taps,
            i,
            o,
            || Complex {
                im: T::zero(),
                re: T::zero(),
            },
            |accum, sample, tap| Complex {
                re: accum.re + sample.re * tap,
                im: accum.im + sample.im * tap,
            },
        )
    }
}

impl<TA: Taps<TapType = Complex<T>>, T> FirKernel<Complex<T>, Complex<T>>
    for FirFilter<Complex<T>, Complex<T>, TA, Complex<T>>
where
    T: Float + Send + Sync + Copy + Zero,
{
    fn filter(&self, i: &[Complex<T>], o: &mut [Complex<T>]) -> (usize, usize, ComputationStatus) {
        fir_kernel_core(
            &self.taps,
            i,
            o,
            || Complex {
                im: T::zero(),
                re: T::zero(),
            },
            |accum, sample, tap| accum + sample * tap,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_fir_kernel() {
        let taps: [f32; 3] = [1.0, 2.0, 3.0];
        let fir = FirFilter::new(taps);
        let input = [1.0, 2.0, 3.0];
        let mut output = [0.0; 3];
        assert_eq!(
            fir.filter(&input, &mut output),
            (1, 1, ComputationStatus::InsufficientInput)
        );
        assert_eq!(output[0], 10.0);

        let mut output = [];
        assert_eq!(
            fir.filter(&input, &mut output),
            (0, 0, ComputationStatus::InsufficientOutput)
        );

        let mut output = [0.0; 3];
        assert_eq!(
            fir.filter(&input, &mut output),
            (1, 1, ComputationStatus::InsufficientInput)
        );
        assert_eq!(output[0], 10.0);

        let input = [1.0, 2.0, 3.0, 4.0, 5.0];
        let mut output = [0.0; 2];
        assert_eq!(
            fir.filter(&input, &mut output),
            (2, 2, ComputationStatus::InsufficientOutput)
        );
        assert_eq!(output[0], 10.0);
        assert_eq!(output[1], 16.0);
    }

    /// Tests the "terminating condition" where the input is finished and the
    /// kernel has produced everything it can given the input, and has exactly
    /// filled the output buffer.
    #[test]
    fn terminating_condition() {
        let taps: [f32; 2] = [1.0, 2.0];
        let fir = FirFilter::new(taps);

        // With 5 input samples and 3 out, we just need more output space
        let input = [1.0, 2.0, 3.0, 4.0, 5.0];
        let mut output = [0.0; 3];
        assert_eq!(
            fir.filter(&input, &mut output),
            (3, 3, ComputationStatus::InsufficientOutput)
        );

        // With 4 input samples and 3 out, we've exactly filled the output
        let input = [1.0, 2.0, 3.0, 4.0];
        let mut output = [0.0; 3];
        assert_eq!(
            fir.filter(&input, &mut output),
            (3, 3, ComputationStatus::BothSufficient)
        );
    }

    #[test]
    fn terminating_condition_f64() {
        let taps: [f64; 2] = [1.0, 2.0];
        let fir = FirFilter::new(taps);

        // With 5 input samples and 3 out, we just need more output space
        let input = [1.0, 2.0, 3.0, 4.0, 5.0];
        let mut output = [0.0; 3];
        assert_eq!(
            fir.filter(&input, &mut output),
            (3, 3, ComputationStatus::InsufficientOutput)
        );

        // With 4 input samples and 3 out, we've exactly filled the output
        let input = [1.0, 2.0, 3.0, 4.0];
        let mut output = [0.0; 3];
        assert_eq!(
            fir.filter(&input, &mut output),
            (3, 3, ComputationStatus::BothSufficient)
        );
    }
}
