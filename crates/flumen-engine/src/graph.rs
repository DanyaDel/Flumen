// Контекст обработки аудио (темп, время, и т.д.)
pub struct ProcessContext {
    pub sample_rate: f32,
    pub buffer_size: usize,
    pub bpm: f32,
}

pub trait AudioNode: Send + std::any::Any {
    fn process(&mut self, outputs: &mut [&mut [f32]], context: &ProcessContext);
    fn trigger(&mut self) { }
    fn trigger_freq(&mut self, _freq: f32) { 
        self.trigger(); // Default fallback
    }
    fn release(&mut self) { }
    fn release_freq(&mut self, _freq: f32) {
        self.release(); // Default fallback
    }
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

impl From<flumen_common::project::Waveform> for Waveform {
    fn from(w: flumen_common::project::Waveform) -> Self {
        match w {
            flumen_common::project::Waveform::Sine => Waveform::Sine,
            flumen_common::project::Waveform::Saw => Waveform::Saw,
            flumen_common::project::Waveform::Square => Waveform::Square,
            flumen_common::project::Waveform::Triangle => Waveform::Triangle,
        }
    }
}

impl From<Waveform> for flumen_common::project::Waveform {
    fn from(w: Waveform) -> Self {
        match w {
            Waveform::Sine => flumen_common::project::Waveform::Sine,
            Waveform::Saw => flumen_common::project::Waveform::Saw,
            Waveform::Square => flumen_common::project::Waveform::Square,
            Waveform::Triangle => flumen_common::project::Waveform::Triangle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdsrEnvelope {
    pub attack: f32,  // seconds
    pub decay: f32,   // seconds
    pub sustain: f32, // 0.0 to 1.0
    pub release: f32, // seconds
}

impl Default for AdsrEnvelope {
    fn default() -> Self {
        Self { attack: 0.01, decay: 0.1, sustain: 0.5, release: 0.2 }
    }
}

impl From<flumen_common::project::AdsrEnvelope> for AdsrEnvelope {
    fn from(e: flumen_common::project::AdsrEnvelope) -> Self {
        Self {
            attack: e.attack,
            decay: e.decay,
            sustain: e.sustain,
            release: e.release,
        }
    }
}

impl From<AdsrEnvelope> for flumen_common::project::AdsrEnvelope {
    fn from(e: AdsrEnvelope) -> Self {
        Self {
            attack: e.attack,
            decay: e.decay,
            sustain: e.sustain,
            release: e.release,
        }
    }
}

pub struct SynthVoice {
    pub waveform: Waveform,
    pub frequency: f32,
    pub base_frequency: f32,
    pub detune: f32, // in cents or multiplier
    pub adsr: AdsrEnvelope,
    pub envelope_val: f32,
    pub release_start_val: f32,
    
    phase: f32,
    state_time: f32,
    is_triggered: bool,
    releasing: bool,
}

impl SynthVoice {
    pub fn new(freq: f32, waveform: Waveform) -> Self {
        Self {
            waveform,
            frequency: freq,
            base_frequency: freq,
            detune: 0.0,
            adsr: AdsrEnvelope::default(),
            phase: 0.0,
            envelope_val: 0.0,
            release_start_val: 0.0,
            state_time: 0.0,
            is_triggered: false,
            releasing: false,
        }
    }

    fn sample_wave(&self, phase: f32) -> f32 {
        match self.waveform {
            Waveform::Sine => phase.sin(),
            Waveform::Saw => 1.0 - (phase / std::f32::consts::PI),
            Waveform::Square => if phase < std::f32::consts::PI { 1.0 } else { -1.0 },
            Waveform::Triangle => {
                let p = phase / (2.0 * std::f32::consts::PI);
                if p < 0.25 { 4.0 * p }
                else if p < 0.75 { 2.0 - 4.0 * p }
                else { -4.0 + 4.0 * p }
            }
        }
    }
}

impl AudioNode for SynthVoice {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn release(&mut self) {
        if !self.releasing {
            self.releasing = true;
            self.state_time = 0.0;
            self.release_start_val = self.envelope_val;
        }
    }

    fn trigger(&mut self) {
        self.is_triggered = true;
        self.releasing = false;
        self.state_time = 0.0;
        // self.phase = 0.0; // Opt-out phase reset for 'analog' feel
    }

    fn trigger_freq(&mut self, freq: f32) {
        self.frequency = freq;
        self.trigger();
    }

    fn process(&mut self, outputs: &mut [&mut [f32]], context: &ProcessContext) {
        let phase_inc = self.frequency * 2.0 * std::f32::consts::PI / context.sample_rate;
        
        for output in outputs {
            for sample in output.iter_mut() {
                // Envelope logic
                if self.is_triggered {
                    if !self.releasing {
                        if self.state_time < self.adsr.attack {
                            self.envelope_val = self.state_time / self.adsr.attack;
                        } else if self.state_time < (self.adsr.attack + self.adsr.decay) {
                            let t = (self.state_time - self.adsr.attack) / self.adsr.decay;
                            self.envelope_val = 1.0 + t * (self.adsr.sustain - 1.0);
                        } else {
                            // Automatic release handled by sequencer
                            self.envelope_val = self.adsr.sustain;
                        }
                    } else {
                        // Release
                        if self.state_time < self.adsr.release {
                            self.envelope_val = self.release_start_val * (1.0 - self.state_time / self.adsr.release);
                        } else {
                            self.envelope_val = 0.0;
                            self.is_triggered = false;
                        }
                    }
                    self.state_time += 1.0 / context.sample_rate;
                }

                *sample = self.sample_wave(self.phase) * 0.1 * self.envelope_val;
                self.phase = (self.phase + phase_inc) % (2.0 * std::f32::consts::PI);
            }
        }
    }
}

impl From<flumen_common::project::SynthParams> for SynthVoice {
    fn from(p: flumen_common::project::SynthParams) -> Self {
        let mut s = Self::new(440.0, p.waveform.into());
        s.adsr = p.adsr.into();
        s
    }
}

impl From<&SynthVoice> for flumen_common::project::SynthParams {
    fn from(s: &SynthVoice) -> Self {
        Self {
            waveform: s.waveform.into(),
            adsr: flumen_common::project::AdsrEnvelope::from(s.adsr),
            unison_count: 1,
            unison_detune: 0.1,
            unison_blend: 0.5,
            octave_offset: 0,
        }
    }
}

pub struct PolySynth {
    pub voices: Vec<SynthVoice>,
    pub waveform: Waveform,
    pub adsr: AdsrEnvelope,
    pub unison_count: u32,
    pub unison_detune: f32,
    pub unison_blend: f32,
    pub octave_offset: i32,
}

impl PolySynth {
    pub fn new(waveform: Waveform) -> Self {
        let mut voices = Vec::new();
        for _ in 0..64 {
            voices.push(SynthVoice::new(440.0, waveform));
        }
        Self {
            voices,
            waveform,
            adsr: AdsrEnvelope::default(),
            unison_count: 1,
            unison_detune: 0.1,
            unison_blend: 0.5,
            octave_offset: 0,
        }
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
        for v in &mut self.voices {
            v.waveform = waveform;
        }
    }

    pub fn set_adsr(&mut self, adsr: AdsrEnvelope) {
        self.adsr = adsr;
        for v in &mut self.voices {
            v.adsr = adsr;
        }
    }
}

impl AudioNode for PolySynth {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn trigger_freq(&mut self, freq: f32) {
        let octave_multiplier = 2.0_f32.powi(self.octave_offset);
        let base_f = freq * octave_multiplier;

        let u_count = self.unison_count.max(1).min(7);
        
        for i in 0..u_count {
            // Detune formula: map i to range [-spread, +spread]
            let spread = if u_count > 1 {
                (i as f32 / (u_count - 1) as f32 - 0.5) * 2.0 * self.unison_detune
            } else {
                0.0
            };
            
            // freq * 2^(cents/1200) -> for simplicity using frequency ratio spread
            let detuned_f = base_f * (1.0 + spread * 0.05); // 5% max spread roughly

            if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_triggered) {
                voice.frequency = detuned_f;
                voice.base_frequency = freq; // store the original trigger freq for release
                voice.trigger();
            }
        }
    }

    fn release_freq(&mut self, freq: f32) {
        for v in &mut self.voices {
            // Check if base frequency matches the one we want to release
            if v.is_triggered && (v.base_frequency - freq).abs() < 0.1 {
                v.release();
            }
        }
    }

    fn release(&mut self) {
        for v in &mut self.voices {
            v.release();
        }
    }

    fn process(&mut self, outputs: &mut [&mut [f32]], context: &ProcessContext) {
        let n = outputs[0].len();
        let mut temp_buf = vec![0.0; n];
        
        for v in &mut self.voices {
            if v.is_triggered || v.releasing {
                for s in &mut temp_buf { *s = 0.0; }
                let mut temp_ptr = [&mut temp_buf[..]];
                v.process(&mut temp_ptr, context);
                
                // Unison Blend logic: scale volume of detuned voices
                let octave_multiplier = 2.0_f32.powi(self.octave_offset);
                let is_center = (v.frequency - v.base_frequency * octave_multiplier).abs() < 0.1;
                let vol = if is_center { 1.0 } else { self.unison_blend };

                for (out_sample, voice_sample) in outputs[0].iter_mut().zip(temp_buf.iter()) {
                    *out_sample += *voice_sample * vol;
                }
            }
        }
    }
}


/// Состояние дорожки в движке (120 нот x 128 шагов)
#[derive(Clone, Copy, Debug)]
pub struct Pattern {
    pub grid: [[u8; 128]; 120], // 0: no note, >0: note length in steps
}

#[derive(Clone, Copy, Debug)]
pub struct ArrangementItem {
    pub track_id: u32,
    pub pattern_index: usize,
    pub start_step: u32,
    pub length: u32,
}

impl Default for Pattern {
    fn default() -> Self {
        Self { grid: [[0; 128]; 120] }
    }
}

impl From<flumen_common::project::Pattern> for Pattern {
    fn from(p: flumen_common::project::Pattern) -> Self {
        let mut grid = [[0; 128]; 120];
        for (r, row) in p.grid.iter().enumerate().take(120) {
            for (c, &val) in row.iter().enumerate().take(128) {
                grid[r][c] = val;
            }
        }
        Self { grid }
    }
}

impl From<&Pattern> for flumen_common::project::Pattern {
    fn from(p: &Pattern) -> Self {
        let mut grid = vec![vec![0; 128]; 120];
        for r in 0..120 {
            for c in 0..128 {
                grid[r][c] = p.grid[r][c];
            }
        }
        Self { grid }
    }
}

#[derive(Clone, Debug)]
pub struct AutomationPoint {
    pub step: f32,
    pub value: f32,
}

#[derive(Clone, Debug)]
pub struct AutomationLane {
    pub param_name: String,
    pub points: Vec<AutomationPoint>,
}

impl AutomationLane {
    pub fn new(param_name: &str) -> Self {
        Self {
            param_name: param_name.to_string(),
            points: Vec::new(),
        }
    }

    pub fn add_point(&mut self, step: f32, value: f32) {
        self.points.push(AutomationPoint { step, value });
        self.points.sort_by(|a, b| a.step.partial_cmp(&b.step).unwrap());
    }

    pub fn remove_point(&mut self, step: f32, tolerance: f32) -> bool {
        if let Some(pos) = self.points.iter().position(|p| (p.step - step).abs() < tolerance) {
            self.points.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn get_value(&self, step: f32) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }

        if step <= self.points[0].step {
            return self.points[0].value;
        }

        if step >= self.points.last().unwrap().step {
            return self.points.last().unwrap().value;
        }

        for i in 0..self.points.len() - 1 {
            let p0 = &self.points[i];
            let p1 = &self.points[i + 1];
            if step >= p0.step && step <= p1.step {
                let t = (step - p0.step) / (p1.step - p0.step);
                return p0.value + t * (p1.value - p0.value);
            }
        }

        0.0
    }
}

pub struct EngineTrack {
    pub node: Box<dyn AudioNode>,
    pub patterns: Vec<Pattern>,
    pub current_pattern_idx: usize,
    pub volume: f32,
    pub pan: f32,
    pub automation: Vec<AutomationLane>,
}

impl EngineTrack {
    pub fn current_pattern(&self) -> &Pattern {
        &self.patterns[self.current_pattern_idx]
    }
    pub fn current_pattern_mut(&mut self) -> &mut Pattern {
        &mut self.patterns[self.current_pattern_idx]
    }

    pub fn add_pattern(&mut self) -> usize {
        self.patterns.push(Pattern::default());
        let idx = self.patterns.len() - 1;
        self.current_pattern_idx = idx;
        idx
    }

    pub fn duplicate_pattern(&mut self) -> Option<usize> {
        let current = self.patterns[self.current_pattern_idx].clone();
        self.patterns.push(current);
        let idx = self.patterns.len() - 1;
        self.current_pattern_idx = idx;
        Some(idx)
    }

    pub fn delete_pattern(&mut self) -> bool {
        if self.patterns.len() <= 1 {
            return false;
        }
        self.patterns.remove(self.current_pattern_idx);
        if self.current_pattern_idx >= self.patterns.len() {
            self.current_pattern_idx = self.patterns.len() - 1;
        }
        true
    }

    pub fn switch_pattern(&mut self, idx: usize) -> bool {
        if idx < self.patterns.len() {
            self.current_pattern_idx = idx;
            true
        } else {
            false
        }
    }

    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}
 // -1.0 (L) to 1.0 (R)

/// Секвенсор и микшер
pub struct SequencerEngine {
    pub tracks: Vec<EngineTrack>,
    pub playlist: Vec<ArrangementItem>,
    pub current_step: usize,
    pub is_playing: bool,
    pub is_arrangement_mode: bool,
    pub bpm: f32,
    samples_per_step: f32,
    sample_counter: f32,
    // (track_idx, pitch, remaining_steps)
    active_notes: Vec<(usize, usize, u8)>,
}

impl SequencerEngine {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            playlist: Vec::new(),
            current_step: 0,
            is_playing: false,
            is_arrangement_mode: false,
            bpm: 120.0,
            samples_per_step: 0.0,
            sample_counter: 0.0,
            active_notes: Vec::new(),
        }
    }

    pub fn add_track(&mut self, node: Box<dyn AudioNode>) {
        self.tracks.push(EngineTrack {
            node,
            patterns: vec![Pattern::default()],
            current_pattern_idx: 0,
            volume: 0.7,
            pan: 0.0,
            automation: Vec::new(),
        });
    }

    fn update_timing(&mut self, context: &ProcessContext) {
        let beats_per_second = self.bpm / 60.0;
        let samples_per_beat = context.sample_rate / beats_per_second;
        self.samples_per_step = samples_per_beat / 4.0;
    }

    /// Клонирование для рендеринга (без потоков)
    pub fn clone_for_render(&self) -> RenderSequencerEngine {
        RenderSequencerEngine {
            tracks: self.tracks.iter().map(|t| {
                // Для рендера создаём новый синтезатор с теми же параметрами
                let new_node: Box<dyn AudioNode> = if let Some(poly) = t.node.as_any().downcast_ref::<PolySynth>() {
                    let mut new_poly = PolySynth::new(poly.waveform);
                    new_poly.set_adsr(poly.adsr);
                    new_poly.unison_count = poly.unison_count;
                    new_poly.unison_detune = poly.unison_detune;
                    new_poly.unison_blend = poly.unison_blend;
                    new_poly.octave_offset = poly.octave_offset;
                    Box::new(new_poly)
                } else if let Some(voice) = t.node.as_any().downcast_ref::<SynthVoice>() {
                    Box::new(SynthVoice::new(voice.frequency, voice.waveform))
                } else {
                    Box::new(PolySynth::new(Waveform::Sine))
                };
                EngineTrack {
                    node: new_node,
                    patterns: t.patterns.clone(),
                    current_pattern_idx: t.current_pattern_idx,
                    volume: t.volume,
                    pan: t.pan,
                    automation: t.automation.clone(),
                }
            }).collect(),
            playlist: self.playlist.clone(),
            bpm: self.bpm,
        }
    }
}

/// Упрощённая версия SequencerEngine для рендеринга
pub struct RenderSequencerEngine {
    pub tracks: Vec<EngineTrack>,
    pub playlist: Vec<ArrangementItem>,
    pub bpm: f32,
}

pub struct Omnia {
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

    delay_buffer_l: Vec<f32>,
    delay_buffer_r: Vec<f32>,
    delay_ptr: usize,

    rev_buffer: Vec<f32>,
    rev_ptr: usize,

    lp_l: f32, lp_r: f32,
    hp_l: f32, hp_r: f32,
}

impl Clone for Omnia {
    fn clone(&self) -> Self {
        Self {
            delay_time: self.delay_time,
            delay_feedback: self.delay_feedback,
            delay_mix: self.delay_mix,
            reverb_size: self.reverb_size,
            reverb_mix: self.reverb_mix,
            eq_low: self.eq_low,
            eq_high: self.eq_high,
            dist_drive: self.dist_drive,
            dist_mix: self.dist_mix,
            is_enabled: self.is_enabled,
            delay_buffer_l: self.delay_buffer_l.clone(),
            delay_buffer_r: self.delay_buffer_r.clone(),
            delay_ptr: self.delay_ptr,
            rev_buffer: self.rev_buffer.clone(),
            rev_ptr: self.rev_ptr,
            lp_l: self.lp_l,
            lp_r: self.lp_r,
            hp_l: self.hp_l,
            hp_r: self.hp_r,
        }
    }
}

impl Omnia {
    pub fn new() -> Self {
        Self {
            delay_time: 0.3, // seconds
            delay_feedback: 0.4,
            delay_mix: 0.0,
            reverb_size: 0.5,
            reverb_mix: 0.0,
            eq_low: 0.0,
            eq_high: 0.0,
            dist_drive: 0.0,
            dist_mix: 0.0,
            is_enabled: true,

            delay_buffer_l: vec![0.0; 48000 * 2],
            delay_buffer_r: vec![0.0; 48000 * 2],
            delay_ptr: 0,

            rev_buffer: vec![0.0; 48000],
            rev_ptr: 0,

            lp_l: 0.0, lp_r: 0.0,
            hp_l: 0.0, hp_r: 0.0,
        }
    }

    pub fn to_params(&self) -> flumen_common::project::OmniaParams {
        flumen_common::project::OmniaParams {
            delay_time: self.delay_time,
            delay_feedback: self.delay_feedback,
            delay_mix: self.delay_mix,
            reverb_size: self.reverb_size,
            reverb_mix: self.reverb_mix,
            eq_low: self.eq_low,
            eq_high: self.eq_high,
            dist_drive: self.dist_drive,
            dist_mix: self.dist_mix,
            is_enabled: self.is_enabled,
        }
    }

    pub fn apply_params(&mut self, p: flumen_common::project::OmniaParams) {
        self.delay_time = p.delay_time;
        self.delay_feedback = p.delay_feedback;
        self.delay_mix = p.delay_mix;
        self.reverb_size = p.reverb_size;
        self.reverb_mix = p.reverb_mix;
        self.eq_low = p.eq_low;
        self.eq_high = p.eq_high;
        self.dist_drive = p.dist_drive;
        self.dist_mix = p.dist_mix;
        self.is_enabled = p.is_enabled;
    }

    pub fn process(&mut self, l: &mut f32, r: &mut f32, sample_rate: f32) {
        if !self.is_enabled { return; }

        let mut in_l = *l;
        let mut in_r = *r;

        // 1. Distortion (Soft clipping)
        if self.dist_mix > 0.0 {
            let drive = 1.0 + self.dist_drive * 10.0;
            let d_l = (in_l * drive).tanh();
            let d_r = (in_r * drive).tanh();
            in_l = in_l * (1.0 - self.dist_mix) + d_l * self.dist_mix;
            in_r = in_r * (1.0 - self.dist_mix) + d_r * self.dist_mix;
        }

        // 2. EQ (Simple shelving/filters)
        // High shelf approx
        let alpha_hp = 1.0 / (1.0 + sample_rate / (2.0 * std::f32::consts::PI * 5000.0));
        self.hp_l += alpha_hp * (in_l - self.hp_l);
        self.hp_r += alpha_hp * (in_r - self.hp_r);
        in_l += (in_l - self.hp_l) * self.eq_high;
        in_r += (in_r - self.hp_r) * self.eq_high;

        // Low shelf approx
        let alpha_lp = 1.0 / (1.0 + sample_rate / (2.0 * std::f32::consts::PI * 200.0));
        self.lp_l += alpha_lp * (in_l - self.lp_l);
        self.lp_r += alpha_lp * (in_r - self.lp_r);
        in_l += self.lp_l * self.eq_low;
        in_r += self.lp_r * self.eq_low;

        // 3. Delay
        let delay_samples = (self.delay_time * sample_rate).clamp(1.0, self.delay_buffer_l.len() as f32 - 1.0) as usize;
        let read_ptr = (self.delay_ptr + self.delay_buffer_l.len() - delay_samples) % self.delay_buffer_l.len();
        
        let delayed_l = self.delay_buffer_l[read_ptr];
        let delayed_r = self.delay_buffer_r[read_ptr];
        
        self.delay_buffer_l[self.delay_ptr] = in_l + delayed_l * self.delay_feedback;
        self.delay_buffer_r[self.delay_ptr] = in_r + delayed_r * self.delay_feedback;
        self.delay_ptr = (self.delay_ptr + 1) % self.delay_buffer_l.len();
        
        in_l = in_l * (1.0 - self.delay_mix) + delayed_l * self.delay_mix;
        in_r = in_r * (1.0 - self.delay_mix) + delayed_r * self.delay_mix;

        // 4. Reverb (Simplistic)
        if self.reverb_mix > 0.0 {
            let rev_len = (0.05 * self.reverb_size * sample_rate).clamp(100.0, self.rev_buffer.len() as f32 - 1.0) as usize;
            let rev_read = (self.rev_ptr + self.rev_buffer.len() - rev_len) % self.rev_buffer.len();
            let rev_sig = self.rev_buffer[rev_read];
            
            self.rev_buffer[self.rev_ptr] = (in_l + in_r) * 0.5 + rev_sig * 0.7 * self.reverb_size;
            self.rev_ptr = (self.rev_ptr + 1) % self.rev_buffer.len();
            
            in_l += rev_sig * self.reverb_mix;
            in_r += rev_sig * self.reverb_mix;
        }

        *l = in_l;
        *r = in_r;
    }
}

/// Граф обработки аудио (теперь Stereo)
pub struct AudioGraph {
    pub engine: SequencerEngine,
    pub omnia: Omnia,
}

impl AudioGraph {
    pub fn new() -> Self {
        Self { 
            engine: SequencerEngine::new(),
            omnia: Omnia::new(),
        }
    }

    pub fn save_project(&self, name: String) -> flumen_common::project::Project {
        let mut tracks = Vec::new();
        for (i, track) in self.engine.tracks.iter().enumerate() {
            let synth = if let Some(s) = track.node.as_any().downcast_ref::<PolySynth>() {
                 flumen_common::project::SynthParams {
                    waveform: s.waveform.into(),
                    adsr: flumen_common::project::AdsrEnvelope::from(s.adsr),
                    unison_count: s.unison_count,
                    unison_detune: s.unison_detune,
                    unison_blend: s.unison_blend,
                    octave_offset: s.octave_offset,
                 }
            } else if let Some(s) = track.node.as_any().downcast_ref::<SynthVoice>() {
                 flumen_common::project::SynthParams {
                     waveform: s.waveform.into(),
                     adsr: flumen_common::project::AdsrEnvelope::from(s.adsr),
                     unison_count: 1,
                     unison_detune: 0.1,
                     unison_blend: 0.5,
                     octave_offset: 0,
                 }
            } else {
                // Fallback or default if not PolySynth
                flumen_common::project::SynthParams {
                    waveform: flumen_common::project::Waveform::Sine,
                    adsr: flumen_common::project::AdsrEnvelope { attack: 0.01, decay: 0.1, sustain: 0.5, release: 0.2 },
                    unison_count: 1,
                    unison_detune: 0.1,
                    unison_blend: 0.5,
                    octave_offset: 0,
                }
            };

            tracks.push(flumen_common::project::Track {
                id: i as u32,
                name: format!("Track {}", i + 1),
                volume: track.volume,
                panned: track.pan,
                synth,
                patterns: track.patterns.iter().map(|p| p.into()).collect(),
            });
        }

        let mut playlist = Vec::new();
        for item in self.engine.playlist.iter() {
            playlist.push(flumen_common::project::ArrangementItem {
                track_id: item.track_id,
                pattern_index: item.pattern_index,
                start_step: item.start_step,
                length: item.length,
            });
        }

        flumen_common::project::Project {
            name,
            bpm: self.engine.bpm,
            tracks,
            playlist,
            omnia: self.omnia.to_params(),
        }
    }

    pub fn load_project(&mut self, proj: flumen_common::project::Project) {
        self.engine.bpm = proj.bpm;
        self.omnia.apply_params(proj.omnia);
        self.engine.tracks.clear();
        self.engine.active_notes.clear();
        
        for t in proj.tracks {
            let mut track = EngineTrack {
                node: Box::new(PolySynth::new(t.synth.waveform.into())),
                patterns: t.patterns.into_iter().map(|p| p.into()).collect(),
                current_pattern_idx: 0,
                volume: t.volume,
                pan: t.panned,
                automation: Vec::new(),
            };
            if let Some(poly) = track.node.as_any_mut().downcast_mut::<PolySynth>() {
                poly.set_adsr(t.synth.adsr.into());
                poly.unison_count = t.synth.unison_count;
                poly.unison_detune = t.synth.unison_detune;
                poly.unison_blend = t.synth.unison_blend;
                poly.octave_offset = t.synth.octave_offset;
            }
            self.engine.tracks.push(track);
        }

        self.engine.playlist.clear();
        for item in proj.playlist {
            self.engine.playlist.push(ArrangementItem {
                track_id: item.track_id,
                pattern_index: item.pattern_index,
                start_step: item.start_step,
                length: item.length,
            });
        }
    }

    pub fn process(&mut self, context: &ProcessContext, output_buffer: &mut [f32]) {
        self.engine.update_timing(context);
        
        // Assume stereo interleaved [L, R, L, R, ...]
        let num_channels = 2;
        let num_frames = output_buffer.len() / num_channels;
        
        // Временные буферы для L и R каналов (по количеству кадров)
        let mut left_buffer = vec![0.0; num_frames];
        let mut right_buffer = vec![0.0; num_frames];
        let mut node_output = vec![0.0; num_frames];

        // Обработка тайминга (по кадрам)
        if self.engine.is_playing {
            for _f in 0..num_frames {
                // Триггерим текущий шаг
                if self.engine.sample_counter == 0.0 {
                    let current_step = self.engine.current_step;
                    
                    // 1. Update active notes (decrement length and release if finished)
                    let mut i = 0;
                    while i < self.engine.active_notes.len() {
                        self.engine.active_notes[i].2 -= 1;
                        if self.engine.active_notes[i].2 == 0 {
                            let (track_idx, _, _) = self.engine.active_notes.remove(i);
                            if let Some(track) = self.engine.tracks.get_mut(track_idx) {
                                track.node.release();
                            }
                        } else {
                            i += 1;
                        }
                    }

                    // 2. Trigger new notes
                    for (track_idx, track) in self.engine.tracks.iter_mut().enumerate() {
                        let mut new_notes = Vec::new();
                        {
                            let pattern = track.current_pattern();
                            for pitch_idx in 0..120 {
                                let len = pattern.grid[pitch_idx][current_step];
                                if len > 0 {
                                    new_notes.push((pitch_idx, len));
                                }
                            }
                        }

                        for (pitch_idx, len) in new_notes {
                            // MIDI 0 is C-1, so 10 octaves could be midi 12..132 or something.
                            // Let's stick with midi = 12 + pitch_idx (C0)
                            let midi = 12 + pitch_idx as i32;
                            let freq = 440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0);
                            track.node.trigger_freq(freq);
                            
                            // Register as active
                            self.engine.active_notes.push((track_idx, pitch_idx, len));
                        }
                    }
                }

                self.engine.sample_counter += 1.0;
                if self.engine.sample_counter >= self.engine.samples_per_step {
                    self.engine.sample_counter = 0.0;
                    self.engine.current_step = (self.engine.current_step + 1) % 128;
                }
            }
        } else {
            // Anti-pop: If we just stopped, release all nodes
            if self.engine.sample_counter != -1.0 {
                for track in &mut self.engine.tracks {
                    track.node.release();
                }
                self.engine.active_notes.clear();
                // Use -1.0 as a marker that we've handled the stop
                self.engine.sample_counter = -1.0;
            }
            self.engine.current_step = 0;
        }

        // Рендерим каждую дорожку
        for track in &mut self.engine.tracks {
            for s in node_output.iter_mut() { *s = 0.0; }
            let mut outputs_ptr = [&mut node_output[..]];
            
            // Render node
            track.node.process(&mut outputs_ptr, context);
            
            let pan_l = (1.0 - track.pan).min(1.0).max(0.0).sqrt();
            let pan_r = (1.0 + track.pan).min(1.0).max(0.0).sqrt();
            let vol = track.volume;

            for (i, sample) in node_output.iter().enumerate() {
                left_buffer[i] += sample * vol * pan_l;
                right_buffer[i] += sample * vol * pan_r;
            }
        }
        
        // Интерливинг в выходной буфер
        for i in 0..num_frames {
            let mut l = left_buffer[i];
            let mut r = right_buffer[i];

            // Omnia Master FX
            self.omnia.process(&mut l, &mut r, context.sample_rate);

            // Лимитер
            output_buffer[i * 2] = l.clamp(-1.0, 1.0);
            output_buffer[i * 2 + 1] = r.clamp(-1.0, 1.0);
        }
    }

    /// Клонирование графа для рендеринга (оффлайн экспорт)
    pub fn clone_for_render(&self) -> RenderGraph {
        RenderGraph {
            engine: self.engine.clone_for_render(),
            omnia: self.omnia.clone(),
        }
    }
}

/// Упрощённая версия AudioGraph для оффлайн рендеринга
pub struct RenderGraph {
    pub engine: RenderSequencerEngine,
    pub omnia: Omnia,
}

impl RenderGraph {
    /// Рендеринг одного буфера для экспорта
    pub fn render_buffer(
        &mut self,
        output_buffer: &mut [f32],
        context: &ProcessContext,
        current_step: &mut usize,
        sample_counter: &mut f32,
        samples_per_step: f32,
        active_notes: &mut Vec<(usize, usize, u8)>,
    ) {
        let num_channels = 2;
        let num_frames = output_buffer.len() / num_channels;

        let mut left_buffer = vec![0.0; num_frames];
        let mut right_buffer = vec![0.0; num_frames];
        let mut node_output = vec![0.0; num_frames];

        // Обработка тайминга и триггеров
        for _f in 0..num_frames {
            if *sample_counter == 0.0 {
                // Release завершённых нот
                let mut i = 0;
                while i < active_notes.len() {
                    active_notes[i].2 -= 1;
                    if active_notes[i].2 == 0 {
                        let (track_idx, pitch, _) = active_notes.remove(i);
                        if let Some(track) = self.engine.tracks.get_mut(track_idx) {
                            // Вычисляем частоту для релиза
                            let midi = 12 + pitch as i32;
                            let freq = 440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0);
                            track.node.release_freq(freq);
                        }
                    } else {
                        i += 1;
                    }
                }

                // Trigger новых нот — сначала собираем, потом триггерим
                let mut notes_to_trigger: Vec<(usize, usize, f32, u8)> = Vec::new();
                for (track_idx, track) in self.engine.tracks.iter().enumerate() {
                    let pattern = track.current_pattern();
                    for pitch_idx in 0..120 {
                        let len = pattern.grid[pitch_idx][*current_step];
                        if len > 0 {
                            let midi = 12 + pitch_idx as i32;
                            let freq = 440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0);
                            notes_to_trigger.push((track_idx, pitch_idx, freq, len));
                        }
                    }
                }

                // Теперь триггерим
                for (track_idx, pitch_idx, freq, len) in notes_to_trigger {
                    if let Some(track) = self.engine.tracks.get_mut(track_idx) {
                        track.node.trigger_freq(freq);
                    }
                    active_notes.push((track_idx, pitch_idx, len));
                }
            }

            *sample_counter += 1.0;
            if *sample_counter >= samples_per_step {
                *sample_counter = 0.0;
                *current_step = (*current_step + 1) % 128;
            }
        }

        // Рендеринг дорожек
        for track in &mut self.engine.tracks {
            for s in node_output.iter_mut() {
                *s = 0.0;
            }

            let mut outputs_ptr = [&mut node_output[..]];
            track.node.process(&mut outputs_ptr, context);

            let pan_l = (1.0 - track.pan).min(1.0).max(0.0).sqrt();
            let pan_r = (1.0 + track.pan).min(1.0).max(0.0).sqrt();
            let vol = track.volume;

            for (i, sample) in node_output.iter().enumerate() {
                left_buffer[i] += sample * vol * pan_l;
                right_buffer[i] += sample * vol * pan_r;
            }
        }

        // Интерливинг и FX
        for i in 0..num_frames {
            let mut l = left_buffer[i];
            let mut r = right_buffer[i];

            self.omnia.process(&mut l, &mut r, context.sample_rate);

            output_buffer[i * 2] = l.clamp(-1.0, 1.0);
            output_buffer[i * 2 + 1] = r.clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48000.0;
    const BUFFER_SIZE: usize = 480;

    fn create_context() -> ProcessContext {
        ProcessContext {
            sample_rate: SAMPLE_RATE,
            buffer_size: BUFFER_SIZE,
            bpm: 120.0,
        }
    }

    #[test]
    fn test_sine_wave_no_clipping() {
        let mut voice = SynthVoice::new(440.0, Waveform::Sine);
        voice.trigger();
        
        let mut output = vec![0.0; BUFFER_SIZE];
        let ctx = create_context();
        
        voice.process(&mut [&mut output], &ctx);
        
        let has_signal = output.iter().any(|&s| s.abs() > 0.01);
        assert!(has_signal, "Sine wave должна генерировать сигнал");
        
        for (i, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= 1.0,
                "Клиппинг на сэмпле #{}: {}", i, sample
            );
        }
    }

    #[test]
    fn test_all_waveforms_no_clipping() {
        let waveforms = [Waveform::Sine, Waveform::Saw, Waveform::Square, Waveform::Triangle];
        
        for waveform in waveforms {
            let mut voice = SynthVoice::new(440.0, waveform);
            voice.trigger();
            
            let mut output = vec![0.0; BUFFER_SIZE];
            let ctx = create_context();
            voice.process(&mut [&mut output], &ctx);
            
            for (i, &sample) in output.iter().enumerate() {
                assert!(
                    sample.is_finite(),
                    "Некорректное значение для {:?} на сэмпле #{}", waveform, i
                );
                assert!(
                    sample.abs() <= 1.0,
                    "Клиппинг для {:?} на сэмпле #{}: {}", waveform, i, sample
                );
            }
        }
    }

    #[test]
    fn test_adsr_attack_phase() {
        let mut voice = SynthVoice::new(440.0, Waveform::Sine);
        voice.adsr.attack = 0.1;
        voice.adsr.decay = 0.1;
        voice.adsr.sustain = 0.5;
        voice.trigger();
        
        let ctx = create_context();
        let samples_per_ms = SAMPLE_RATE / 1000.0;
        
        let mut prev_envelope = 0.0;
        for _ in 0..(samples_per_ms * 50.0) as usize {
            let mut output = [0.0];
            voice.process(&mut [&mut output], &ctx);
            
            assert!(
                voice.envelope_val >= prev_envelope,
                "Огибающая должна расти во время атаки"
            );
            prev_envelope = voice.envelope_val;
        }
        
        assert!(prev_envelope > 0.3, "После 50ms атаки огибающая должна быть > 0.3 (получено {})", prev_envelope);
    }

    #[test]
    fn test_adsr_release_phase() {
        let mut voice = SynthVoice::new(440.0, Waveform::Sine);
        voice.adsr.attack = 0.01;
        voice.adsr.release = 0.1;
        voice.trigger();
        
        let ctx = create_context();
        
        for _ in 0..1000 {
            let mut output = [0.0];
            voice.process(&mut [&mut output], &ctx);
        }
        
        let peak_envelope = voice.envelope_val;
        voice.release();
        
        let mut prev_envelope = voice.envelope_val;
        for _ in 0..100 {
            let mut output = [0.0];
            voice.process(&mut [&mut output], &ctx);
            
            assert!(
                voice.envelope_val <= prev_envelope,
                "Огибающая должна падать во время релиза"
            );
            prev_envelope = voice.envelope_val;
        }
        
        assert!(voice.envelope_val < peak_envelope);
    }

    #[test]
    fn test_voice_deterministic() {
        let mut voice1 = SynthVoice::new(440.0, Waveform::Sine);
        let mut voice2 = SynthVoice::new(440.0, Waveform::Sine);
        
        voice1.trigger();
        voice2.trigger();
        
        let mut output1 = vec![0.0; BUFFER_SIZE];
        let mut output2 = vec![0.0; BUFFER_SIZE];
        let ctx = create_context();
        
        voice1.process(&mut [&mut output1], &ctx);
        voice2.process(&mut [&mut output2], &ctx);
        
        assert_eq!(output1, output2, "Одинаковые голоса должны давать одинаковый выход");
    }

    #[test]
    fn test_polysynth_trigger() {
        let mut synth = PolySynth::new(Waveform::Saw);
        synth.trigger_freq(440.0);
        
        let active_voices = synth.voices.iter().filter(|v| v.is_triggered).count();
        assert!(active_voices > 0, "После trigger_freq хотя бы один голос должен быть активен");
    }

    #[test]
    fn test_polysynth_unison() {
        let mut synth = PolySynth::new(Waveform::Sine);
        synth.unison_count = 3;
        synth.unison_detune = 0.1;
        synth.trigger_freq(440.0);
        
        let active_voices = synth.voices.iter().filter(|v| v.is_triggered).count();
        assert_eq!(active_voices, 3, "Unison должен активировать 3 голоса");
    }

    #[test]
    fn test_polysynth_release_freq() {
        let mut synth = PolySynth::new(Waveform::Sine);
        synth.trigger_freq(440.0);
        synth.trigger_freq(880.0);
        
        synth.release_freq(440.0);
        
        let still_active = synth.voices.iter()
            .filter(|v| v.is_triggered && (v.base_frequency - 880.0).abs() < 1.0)
            .count();
        
        assert!(still_active > 0, "Голоса на 880 Гц должны остаться после release_freq(440)");
    }

    #[test]
    fn test_omnia_distortion() {
        let mut omnia = Omnia::new();
        omnia.dist_drive = 0.8;
        omnia.dist_mix = 1.0;
        
        let mut signal_l = 0.5;
        let mut signal_r = 0.5;
        let original_l = signal_l;
        
        omnia.process(&mut signal_l, &mut signal_r, SAMPLE_RATE);
        
        assert!(
            (signal_l - original_l).abs() > 0.01,
            "Дисторшн должен изменять сигнал"
        );
        assert!(signal_l.abs() <= 1.5, "Выход дисторшна не должен превышать 1.5");
    }

    #[test]
    fn test_omnia_delay() {
        let mut omnia = Omnia::new();
        omnia.delay_time = 0.1;
        omnia.delay_feedback = 0.5;
        omnia.delay_mix = 1.0;

        let mut signal_l = 0.0;
        let mut signal_r = 0.0;
        let mut output_history = Vec::new();

        for i in 0..6000 {
            if i == 0 {
                signal_l = 1.0;
                signal_r = 1.0;
            } else {
                signal_l = 0.0;
                signal_r = 0.0;
            }
            omnia.process(&mut signal_l, &mut signal_r, SAMPLE_RATE);
            output_history.push(signal_l);
        }

        // Delay должен создать повторение через ~0.1 сек = ~4800 сэмплов
        let has_delay_repeat = output_history.iter()
            .skip(4000)
            .any(|&v| v.abs() > 0.001);

        assert!(
            has_delay_repeat,
            "Delay должен создавать повторения сигнала (max был {})",
            output_history.iter().skip(4000).map(|&v| v.abs()).fold(0.0, f32::max)
        );
    }

    #[test]
    fn test_omnia_bypass() {
        let mut omnia = Omnia::new();
        omnia.is_enabled = false;
        
        let mut signal_l = 0.5;
        let mut signal_r = 0.5;
        let original_l = signal_l;
        let original_r = signal_r;
        
        omnia.process(&mut signal_l, &mut signal_r, SAMPLE_RATE);
        
        assert_eq!(signal_l, original_l, "В bypass режиме сигнал не должен изменяться");
        assert_eq!(signal_r, original_r, "В bypass режиме сигнал не должен изменяться");
    }

    #[test]
    fn test_omnia_no_clipping() {
        let mut omnia = Omnia::new();
        omnia.dist_drive = 1.0;
        omnia.delay_feedback = 0.9;
        omnia.reverb_mix = 1.0;
        
        let mut signal_l = 1.0;
        let mut signal_r = 1.0;
        
        for _ in 0..1000 {
            omnia.process(&mut signal_l, &mut signal_r, SAMPLE_RATE);
            assert!(signal_l.abs() <= 2.0, "Выход Omnia не должен превышать 2.0 (L)");
            assert!(signal_r.abs() <= 2.0, "Выход Omnia не должен превышать 2.0 (R)");
        }
    }

    #[test]
    fn test_pattern_default_empty() {
        let pattern = Pattern::default();
        
        for pitch in 0..120 {
            for step in 0..128 {
                assert_eq!(pattern.grid[pitch][step], 0, "Паттерн по умолчанию должен быть пустым");
            }
        }
    }

    #[test]
    fn test_pattern_conversion() {
        let mut common_pattern = flumen_common::project::Pattern {
            grid: vec![vec![0; 128]; 120],
        };
        common_pattern.grid[60][0] = 16;
        
        let engine_pattern = Pattern::from(common_pattern.clone());
        let back_to_common = flumen_common::project::Pattern::from(&engine_pattern);
        
        assert_eq!(back_to_common.grid[60][0], 16, "Конверсия должна сохранять данные паттерна");
    }

    #[test]
    fn test_zero_frequency() {
        let mut voice = SynthVoice::new(0.0, Waveform::Sine);
        voice.trigger();
        
        let mut output = vec![0.0; BUFFER_SIZE];
        let ctx = create_context();
        voice.process(&mut [&mut output], &ctx);
        
        for (i, &sample) in output.iter().enumerate() {
            assert!(
                sample.is_finite(),
                "Некорректное значение при частоте 0 Гц на сэмпле #{}", i
            );
        }
    }

    #[test]
    fn test_very_high_frequency() {
        let mut voice = SynthVoice::new(20000.0, Waveform::Sine);
        voice.trigger();
        
        let mut output = vec![0.0; BUFFER_SIZE];
        let ctx = create_context();
        voice.process(&mut [&mut output], &ctx);
        
        for (i, &sample) in output.iter().enumerate() {
            assert!(
                sample.is_finite(),
                "Некорректное значение при 20 kHz на сэмпле #{}", i
            );
        }
    }

    #[test]
    fn test_adsr_zero_values() {
        let mut voice = SynthVoice::new(440.0, Waveform::Sine);
        voice.adsr.attack = 0.0;
        voice.adsr.decay = 0.0;
        voice.adsr.release = 0.0;
        voice.trigger();
        
        let mut output = vec![0.0; BUFFER_SIZE];
        let ctx = create_context();
        
        voice.process(&mut [&mut output], &ctx);
        
        for (i, &sample) in output.iter().enumerate() {
            assert!(
                sample.is_finite(),
                "Некорректное значение при нулевых ADSR на сэмпле #{}", i
            );
        }
    }

    #[test]
    fn test_empty_buffer_processing() {
        let mut voice = SynthVoice::new(440.0, Waveform::Sine);
        voice.trigger();
        
        let mut output = vec![];
        let ctx = create_context();
        
        voice.process(&mut [&mut output], &ctx);
    }

    #[test]
    fn test_sequencer_step_timing() {
        let mut engine = SequencerEngine::new();
        engine.bpm = 120.0;
        engine.is_playing = true;
        
        let ctx = ProcessContext {
            sample_rate: SAMPLE_RATE,
            buffer_size: BUFFER_SIZE,
            bpm: 120.0,
        };
        
        engine.update_timing(&ctx);
        
        let expected_samples_per_step = (SAMPLE_RATE / 2.0) / 4.0;
        
        assert!(
            (engine.samples_per_step - expected_samples_per_step).abs() < 1.0,
            "Неверный расчёт сэмплов на шаг: ожидалось {}, получено {}",
            expected_samples_per_step,
            engine.samples_per_step
        );
    }
}

