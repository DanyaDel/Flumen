pub mod graph;

pub mod engine {
    use crate::graph::{AudioGraph, ProcessContext, PolySynth, Waveform};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::{Arc, Mutex};

    pub struct AudioEngine {
        pub graph: Arc<Mutex<AudioGraph>>,
        sample_rate: f32,
        stream: Option<cpal::Stream>,
    }

    impl AudioEngine {
        pub fn new() -> Self {
            let mut graph = AudioGraph::new();
            
            // Track 1: Bass (Saw)
            graph.engine.add_track(Box::new(PolySynth::new(Waveform::Saw)));
            {
                let pattern = graph.engine.tracks[0].current_pattern_mut();
                // A simple melodic bassline
                pattern.grid[36 - 12][0] = 16;  // C2, length 16 steps
                pattern.grid[36 - 12][4] = 4;   // C2
                pattern.grid[43 - 12][8] = 4;   // G2
                pattern.grid[48 - 12][12] = 4;  // C3
            }

            // Track 2: Lead (Square)
            graph.engine.add_track(Box::new(PolySynth::new(Waveform::Square)));
            {
                let pattern = graph.engine.tracks[1].current_pattern_mut();
                // A simple melody
                pattern.grid[48 - 12][2] = 4;   // C3
                pattern.grid[50 - 12][6] = 4;   // D3
                pattern.grid[52 - 12][10] = 4;  // E3
                pattern.grid[53 - 12][14] = 4;  // F3
            }
            graph.engine.tracks[1].pan = 0.5;

            Self { 
                graph: Arc::new(Mutex::new(graph)),
                sample_rate: 48000.0,
                stream: None,
            }
        }

        pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            let host = cpal::default_host();
            let device = host.default_output_device().ok_or("No output device found")?;
            let config = device.default_output_config()?;
            let sample_rate = config.sample_rate().0 as f32;
            self.sample_rate = sample_rate;

            let graph = self.graph.clone();
            
            let stream = device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut graph = graph.lock().unwrap();
                    let context = ProcessContext {
                        sample_rate,
                        buffer_size: data.len(),
                        bpm: graph.engine.bpm,
                    };
                    
                    // Очищаем буфер перед заполнением
                    for sample in data.iter_mut() { *sample = 0.0; }
                    
                    graph.process(&context, data);
                },
                |err| eprintln!("Audio stream error: {}", err),
                None
            )?;

            stream.play()?;
            self.stream = Some(stream);
            
            println!("Audio Engine (Flumen) started on {} at {}Hz", device.name()?, sample_rate);
            Ok(())
        }
    }
}
