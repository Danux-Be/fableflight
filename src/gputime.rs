//! gpu time per render stage, on with FABLE_GPU_TIMES=1, logged every 60 frames.

const SLOTS: u32 = 32;

pub struct GpuTimer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    period: f64,
    labels: Vec<&'static str>,
    acc: Vec<(&'static str, f64)>,
    frames: u32,
}

impl GpuTimer {
    /// the device features needed, if the env var asks for timing and the adapter has them.
    pub fn features(adapter: &wgpu::Adapter) -> wgpu::Features {
        let f = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if std::env::var("FABLE_GPU_TIMES").is_ok_and(|v| v == "1") && adapter.features().contains(f) { f } else { wgpu::Features::empty() }
    }

    pub fn new(d: &wgpu::Device, q: &wgpu::Queue) -> Option<Self> {
        if !d.features().contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS) {
            return None;
        }
        let size = SLOTS as u64 * 8;
        let set = d.create_query_set(&wgpu::QuerySetDescriptor { label: Some("gpu timer"), ty: wgpu::QueryType::Timestamp, count: SLOTS });
        let resolve = d.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let read = d.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        log::info!("gpu stage timing on");
        Some(Self { set, resolve, read, period: q.get_timestamp_period() as f64, labels: Vec::new(), acc: Vec::new(), frames: 0 })
    }

    /// stamp the end of a stage.
    pub fn mark(&mut self, enc: &mut wgpu::CommandEncoder, label: &'static str) {
        if (self.labels.len() as u32) < SLOTS {
            enc.write_timestamp(&self.set, self.labels.len() as u32);
            self.labels.push(label);
        }
    }

    /// copy the stamps out before submit.
    pub fn resolve(&mut self, enc: &mut wgpu::CommandEncoder) {
        let n = self.labels.len() as u32;
        if n > 1 {
            enc.resolve_query_set(&self.set, 0..n, &self.resolve, 0);
            enc.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, n as u64 * 8);
        }
    }

    /// read the stamps after submit, this waits for the gpu so it is for profiling only.
    pub fn collect(&mut self, d: &wgpu::Device) {
        let n = self.labels.len();
        if n > 1 {
            self.read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            let _ = d.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            {
                let data = self.read.slice(..).get_mapped_range().expect("mapped");
                let ts: &[u64] = bytemuck::cast_slice(&data[..n * 8]);
                for i in 1..n {
                    let ms = ts[i].saturating_sub(ts[i - 1]) as f64 * self.period / 1e6;
                    match self.acc.iter_mut().find(|a| a.0 == self.labels[i]) {
                        Some(a) => a.1 += ms,
                        None => self.acc.push((self.labels[i], ms)),
                    }
                }
            }
            self.read.unmap();
            self.frames += 1;
        }
        self.labels.clear();
        if self.frames >= 60 {
            let k = 1.0 / self.frames as f64;
            let s: Vec<String> = self.acc.iter().map(|(l, ms)| format!("{l} {:.1}", ms * k)).collect();
            log::info!("gpu ms: {}", s.join(", "));
            self.acc.clear();
            self.frames = 0;
        }
    }
}
