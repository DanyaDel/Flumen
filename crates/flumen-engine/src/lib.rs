pub mod graph;

pub mod engine {
    use crate::graph::{AudioGraph, ProcessContext, PolySynth, Waveform};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use hound::{WavSpec, WavWriter};
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

        /// Экспорт проекта в WAV файл
        /// 
        /// # Arguments
        /// * `path` - Путь к файлу (например, "output.wav")
        /// * `duration_secs` - Длительность экспорта в секундах
        pub fn export_to_wav(
            &self,
            path: &str,
            duration_secs: f32,
        ) -> Result<(), Box<dyn std::error::Error>> {
            println!("Начало экспорта в WAV: {} ({} сек)", path, duration_secs);

            let sample_rate: usize = 48000;
            let num_channels: usize = 2;
            let total_samples = (sample_rate as f32 * duration_secs) as usize;
            let buffer_size: usize = 480; // 10ms буферы

            // Создаём спецификацию WAV
            let spec = WavSpec {
                channels: num_channels as u16,
                sample_rate: sample_rate as u32,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };

            let mut writer = WavWriter::create(path, spec)?;

            // Получаем BPM из графа
            let bpm = {
                let graph = self.graph.lock().unwrap();
                graph.engine.bpm
            };

            // Создаём рендер-версию графа
            let mut render_graph = {
                let graph = self.graph.lock().unwrap();
                graph.clone_for_render()
            };

            let mut rendered_samples = 0;
            let mut current_step = 0;
            let mut sample_counter = 0.0;
            let mut active_notes: Vec<(usize, usize, u8)> = Vec::new();

            // Расчёт timing
            let beats_per_second = bpm / 60.0;
            let samples_per_beat = sample_rate as f32 / beats_per_second;
            let samples_per_step = samples_per_beat / 4.0;

            println!("Рендеринг аудио... ({} сэмплов, BPM: {})", total_samples, bpm);

            while rendered_samples < total_samples {
                let mut buffer = vec![0.0f32; buffer_size * num_channels];
                let context = ProcessContext {
                    sample_rate: sample_rate as f32,
                    buffer_size,
                    bpm,
                };

                // Обработка одного буфера
                render_graph.render_buffer(
                    &mut buffer,
                    &context,
                    &mut current_step,
                    &mut sample_counter,
                    samples_per_step,
                    &mut active_notes,
                );

                // Запись в WAV
                for sample in buffer.iter() {
                    if rendered_samples >= total_samples {
                        break;
                    }
                    writer.write_sample(*sample)?;
                    rendered_samples += 1;
                }

                // Прогресс
                if rendered_samples % (sample_rate * 5) == 0 {
                    let progress = (rendered_samples as f32 / total_samples as f32) * 100.0;
                    println!("Прогресс: {:.1}%", progress);
                }
            }

            writer.finalize()?;
            println!("Экспорт завершён: {}", path);
            Ok(())
        }
    }
}
