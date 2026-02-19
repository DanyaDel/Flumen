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

pub struct MultiWaveSynth {
    pub waveform: Waveform,
    pub frequency: f32,
    pub detune: f32, // in cents or multiplier
    pub adsr: AdsrEnvelope,
    pub envelope_val: f32,
    pub release_start_val: f32,
    
    phase: f32,
    state_time: f32,
    is_triggered: bool,
    releasing: bool,
}

impl MultiWaveSynth {
    pub fn new(freq: f32, waveform: Waveform) -> Self {
        Self {
            waveform,
            frequency: freq,
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

impl AudioNode for MultiWaveSynth {
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

impl From<flumen_common::project::SynthParams> for MultiWaveSynth {
    fn from(p: flumen_common::project::SynthParams) -> Self {
        let mut s = Self::new(440.0, p.waveform.into());
        s.adsr = p.adsr.into();
        s
    }
}

impl From<&MultiWaveSynth> for flumen_common::project::SynthParams {
    fn from(s: &MultiWaveSynth) -> Self {
        Self {
            waveform: s.waveform.into(),
            adsr: flumen_common::project::AdsrEnvelope::from(s.adsr),
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

pub struct EngineTrack {
    pub node: Box<dyn AudioNode>,
    pub patterns: Vec<Pattern>,
    pub current_pattern_idx: usize,
    pub volume: f32,
    pub pan: f32,
}

impl EngineTrack {
    pub fn current_pattern(&self) -> &Pattern {
        &self.patterns[self.current_pattern_idx]
    }
    pub fn current_pattern_mut(&mut self) -> &mut Pattern {
        &mut self.patterns[self.current_pattern_idx]
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
        });
    }

    fn update_timing(&mut self, context: &ProcessContext) {
        let beats_per_second = self.bpm / 60.0;
        let samples_per_beat = context.sample_rate / beats_per_second;
        self.samples_per_step = samples_per_beat / 4.0;
    }
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
            let synth = if let Some(s) = track.node.as_any().downcast_ref::<MultiWaveSynth>() {
                 flumen_common::project::SynthParams::from(s)
            } else {
                // Fallback or default if not MultiWaveSynth
                flumen_common::project::SynthParams {
                    waveform: flumen_common::project::Waveform::Sine,
                    adsr: flumen_common::project::AdsrEnvelope { attack: 0.01, decay: 0.1, sustain: 0.5, release: 0.2 },
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
            let track = EngineTrack {
                node: Box::new(MultiWaveSynth::from(t.synth)),
                patterns: t.patterns.into_iter().map(|p| p.into()).collect(),
                current_pattern_idx: 0,
                volume: t.volume,
                pan: t.panned,
            };
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
}

