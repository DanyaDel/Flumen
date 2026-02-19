pub mod project {
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub enum Waveform {
        Sine,
        Saw,
        Square,
        Triangle,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct AdsrEnvelope {
        pub attack: f32,
        pub decay: f32,
        pub sustain: f32,
        pub release: f32,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct SynthParams {
        pub waveform: Waveform,
        pub adsr: AdsrEnvelope,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct OmniaParams {
        pub delay_time: f32,
        pub delay_feedback: f32,
        pub delay_mix: f32,
        pub reverb_size: f32,
        pub reverb_mix: f32,
        pub eq_low: f32,
        pub eq_high: f32,
        pub dist_drive: f32,
        pub dist_mix: f32,
        pub is_enabled: bool,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Pattern {
        pub grid: Vec<Vec<u8>>, // 120 notes x 128 steps
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct ArrangementItem {
        pub track_id: u32,
        pub pattern_index: usize,
        pub start_step: u32,
        pub length: u32,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Track {
        pub id: u32,
        pub name: String,
        pub volume: f32,
        pub panned: f32,
        pub synth: SynthParams,
        pub patterns: Vec<Pattern>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Project {
        pub name: String,
        pub bpm: f32,
        pub tracks: Vec<Track>,
        pub playlist: Vec<ArrangementItem>,
        pub omnia: OmniaParams,
    }
}
