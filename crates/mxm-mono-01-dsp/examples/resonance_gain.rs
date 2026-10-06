//! How much level the filter loses as resonance rises.
//!
//! A ladder's DC gain is `1/(1+k)`, so the drop is inherent to the topology rather than a defect.
//! This measures whether ours matches that theory — a larger loss would be a bug.
//!
//! `cargo run -p mxm-mono-01-dsp --example resonance_gain --release`

use mxm_mono_01_dsp::filter::{K_MAX, Ladder};

const SR: f32 = 48_000.0;

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Drives the filter with a sine well inside the passband and measures the output level.
fn passband_level(resonance: f32, cutoff: f32, tone_hz: f32) -> f32 {
    let mut filter = Ladder::default();
    let mut out = Vec::with_capacity(SR as usize);
    for n in 0..(SR as usize) {
        let t = n as f32 / SR;
        let input = 0.2 * (std::f32::consts::TAU * tone_hz * t).sin();
        out.push(filter.process(input, cutoff, resonance, SR));
    }
    // Second half only, so the measurement is of the settled state.
    rms(&out[out.len() / 2..])
}

fn main() {
    println!("Cutoff wide open (18 kHz), 110 Hz tone — the diagnostic in docs/filters/02.\n");
    println!("  resonance     k     level      vs k=0    theory 1/(1+k)");
    let reference = passband_level(0.0, 18_000.0, 110.0);

    for resonance in [0.0f32, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let k = K_MAX * resonance;
        let level = passband_level(resonance, 18_000.0, 110.0);
        let db = 20.0 * (level / reference).log10();
        let theory = 20.0 * (1.0 / (1.0 + k)).log10();
        println!("     {resonance:>5.2}  {k:>5.2}  {level:>8.5}  {db:>8.2} dB   {theory:>8.2} dB");
    }

    println!("\nAt a musical cutoff (1 kHz), 220 Hz tone:\n");
    println!("  resonance     k     level      vs k=0");
    let reference = passband_level(0.0, 1_000.0, 220.0);
    for resonance in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let level = passband_level(resonance, 1_000.0, 220.0);
        let db = 20.0 * (level / reference).log10();
        println!(
            "     {resonance:>5.2}  {:>5.2}  {level:>8.5}  {db:>8.2} dB",
            K_MAX * resonance
        );
    }
}
