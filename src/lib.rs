use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;

// How many hashes do we compute at a time?
pub const BATCH_SIZE: usize = 65536; // Optimized for GPU utilization (was 4096)
pub const MAX_MSG_SIZE: usize = 256;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct TargetHash {
    pub data: [u32; 4],
}

/// A set of buffers for processing one batch
/// Used for double-buffering to overlap CPU and GPU work
struct BufferSet {
    messages_buffer: wgpu::Buffer,
    lengths_buffer: wgpu::Buffer,
    offsets_buffer: wgpu::Buffer,
    result_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl BufferSet {
    fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        target_buffer: &wgpu::Buffer,
        message_count_buffer: &wgpu::Buffer,
        label: &str,
    ) -> Self {
        // Allocate buffers for this set
        let max_message_bytes = MAX_MSG_SIZE * BATCH_SIZE;
        let messages_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label} Messages Buffer")),
            size: max_message_bytes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lengths_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label} Lengths Buffer")),
            size: (BATCH_SIZE * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let offsets_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label} Offsets Buffer")),
            size: (BATCH_SIZE * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let result_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label} Result Buffer")),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label} Staging Buffer")),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group for this buffer set
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label} Bind Group")),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: messages_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lengths_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: offsets_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: target_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: result_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: message_count_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            messages_buffer,
            lengths_buffer,
            offsets_buffer,
            result_buffer,
            staging_buffer,
            bind_group,
        }
    }
}

/// GPU-based MD5 hash cracker with pipelined execution
pub struct GpuCracker {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    supports_timestamps: bool,
    // Double-buffering: two complete buffer sets for pipelining
    buffer_set_a: BufferSet,
    buffer_set_b: BufferSet,
    // Shared buffers (don't need double-buffering)
    target_buffer: wgpu::Buffer,
    message_count_buffer: wgpu::Buffer,
    // Pre-allocated CPU buffers to avoid repeated allocations
    message_data_bytes: Vec<u8>,
    message_lengths: Vec<u32>,
    message_offsets: Vec<u32>,
}

impl GpuCracker {
    /// Initialize the GPU cracker
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create wgpu instance with Vulkan backend (for AMD GPU support)
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;

        println!("Using GPU: {}", adapter.get_info().name);

        // Check if timestamp queries are supported
        let supports_timestamps = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);

        // Request device and queue with timestamp support if available
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("GPU Device"),
                required_features: if supports_timestamps {
                    wgpu::Features::TIMESTAMP_QUERY
                } else {
                    wgpu::Features::empty()
                },
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        // Load the compiled shader
        let shader_bytes = include_bytes!(env!("shader.spv"));
        // Convert to u32 array for SPIR-V
        let mut shader_u32 = Vec::with_capacity(shader_bytes.len() / 4);
        for chunk in shader_bytes.chunks_exact(4) {
            shader_u32.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MD5 Shader"),
            source: wgpu::ShaderSource::SpirV(Cow::Owned(shader_u32)),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MD5 Bind Group Layout"),
            entries: &[
                // Binding 0: Messages buffer (storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 1: Message lengths buffer (storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 2: Message offsets buffer (storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 3: Target hash (storage, read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 4: Result buffer (storage, read-write)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 5: Message count (uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MD5 Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MD5 Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("md5_crack"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Create shared buffers (don't need double-buffering)
        let target_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Target Buffer"),
            size: 16, // 4 u32s = 16 bytes
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let message_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Message Count Buffer"),
            size: 4,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create two complete buffer sets for double-buffering
        let buffer_set_a = BufferSet::new(
            &device,
            &bind_group_layout,
            &target_buffer,
            &message_count_buffer,
            "Set A",
        );
        let buffer_set_b = BufferSet::new(
            &device,
            &bind_group_layout,
            &target_buffer,
            &message_count_buffer,
            "Set B",
        );

        // Pre-allocate CPU-side buffers with capacity for max batch
        let message_data_bytes = Vec::with_capacity(MAX_MSG_SIZE * BATCH_SIZE);
        let message_lengths = Vec::with_capacity(BATCH_SIZE);
        let message_offsets = Vec::with_capacity(BATCH_SIZE);

        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            supports_timestamps,
            buffer_set_a,
            buffer_set_b,
            target_buffer,
            message_count_buffer,
            message_data_bytes,
            message_lengths,
            message_offsets,
        })
    }

    /// Process a batch of messages and check against target hash
    pub fn process_batch(&mut self, messages: &[&str], target_hash: &[u8; 16]) -> Option<usize> {
        // Clear and reuse pre-allocated buffers
        self.message_data_bytes.clear();
        self.message_lengths.clear();
        self.message_offsets.clear();

        let mut current_offset = 0u32;

        // Prepare message data
        for msg in messages {
            let msg_bytes = msg.as_bytes();
            self.message_offsets.push(current_offset);
            self.message_lengths.push(msg_bytes.len() as u32);
            self.message_data_bytes.extend_from_slice(msg_bytes);
            current_offset += msg_bytes.len() as u32;
        }

        // Pad arrays to BATCH_SIZE
        while self.message_lengths.len() < BATCH_SIZE {
            self.message_offsets.push(current_offset);
            self.message_lengths.push(0);
        }

        // Pack bytes into u32 words (GPU shader expects u32 array)
        let aligned_size = (self.message_data_bytes.len() + 3) & !3;
        self.message_data_bytes.resize(aligned_size, 0);
        let messages_u32: &[u32] = bytemuck::cast_slice(&self.message_data_bytes);
        let messages_bytes = bytemuck::cast_slice(messages_u32);

        // Use buffer_set_a for now (will implement pipelining later)
        let buffer_set = &self.buffer_set_a;

        if !messages_bytes.is_empty() {
            self.queue
                .write_buffer(&buffer_set.messages_buffer, 0, messages_bytes);
        } else {
            self.queue
                .write_buffer(&buffer_set.messages_buffer, 0, &[0u8; 4]);
        }

        let lengths_bytes = bytemuck::cast_slice(&self.message_lengths);
        let offsets_bytes = bytemuck::cast_slice(&self.message_offsets);
        self.queue
            .write_buffer(&buffer_set.lengths_buffer, 0, lengths_bytes);
        self.queue
            .write_buffer(&buffer_set.offsets_buffer, 0, offsets_bytes);
        self.queue.write_buffer(&self.target_buffer, 0, target_hash);
        self.queue
            .write_buffer(&buffer_set.result_buffer, 0, &(-1i32).to_le_bytes());
        self.queue.write_buffer(
            &self.message_count_buffer,
            0,
            &(messages.len() as u32).to_le_bytes(),
        );

        // Create command encoder and dispatch compute shader
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("MD5 Command Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("MD5 Crack Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &buffer_set.bind_group, &[]);

            // Dispatch with workgroups based on actual batch size (each workgroup has 64 threads)
            let num_workgroups = (messages.len() as u32).div_ceil(64);
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        // Copy result to staging buffer
        encoder.copy_buffer_to_buffer(
            &buffer_set.result_buffer,
            0,
            &buffer_set.staging_buffer,
            0,
            4,
        );

        // Submit commands
        self.queue.submit(Some(encoder.finish()));

        // Read result
        let buffer_slice = buffer_set.staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        self.device
            .poll(wgpu::PollType::Wait)
            .expect("Failed to poll device");
        receiver.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();
        let result: i32 = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        drop(data);
        buffer_set.staging_buffer.unmap();

        if result >= 0 {
            Some(result as usize)
        } else {
            None
        }
    }

    /// Crack a hash using a wordlist with pipelined execution
    /// Overlaps CPU preparation of batch N+1 with GPU execution of batch N
    pub fn crack(&mut self, target_hash: &[u8; 16], wordlist: &[&str]) -> Option<String> {
        let chunks: Vec<&[&str]> = wordlist.chunks(BATCH_SIZE).collect();
        if chunks.is_empty() {
            return None;
        }

        // Process first batch (no overlap yet) - use buffer set A
        self.prepare_and_submit_batch(false, chunks[0], target_hash);

        // Pipeline: overlap CPU prep of batch N+1 with GPU execution of batch N
        for i in 1..chunks.len() {
            // Alternate between buffer sets (false = A, true = B)
            let use_set_b = i % 2 == 1;

            // While GPU processes current batch, prepare next batch on CPU
            self.prepare_batch(use_set_b, chunks[i], target_hash);

            // Wait for previous batch to complete and check result
            let prev_use_set_b = (i - 1) % 2 == 1;
            if let Some(idx) = self.read_result(prev_use_set_b) {
                return Some(chunks[i - 1][idx].to_string());
            }

            // Submit next batch to GPU (non-blocking)
            self.submit_batch(use_set_b, chunks[i].len());
        }

        // Process last batch result
        let last_use_set_b = (chunks.len() - 1) % 2 == 1;
        if let Some(idx) = self.read_result(last_use_set_b) {
            return Some(chunks[chunks.len() - 1][idx].to_string());
        }

        None
    }

    /// Prepare batch data on CPU and submit to GPU (combined)
    fn prepare_and_submit_batch(
        &mut self,
        use_set_b: bool,
        messages: &[&str],
        target_hash: &[u8; 16],
    ) {
        self.prepare_batch(use_set_b, messages, target_hash);
        self.submit_batch(use_set_b, messages.len());
    }

    /// Prepare batch data on CPU (no GPU submission)
    fn prepare_batch(&mut self, use_set_b: bool, messages: &[&str], target_hash: &[u8; 16]) {
        let buffer_set = if use_set_b {
            &self.buffer_set_b
        } else {
            &self.buffer_set_a
        };
        // Clear and reuse pre-allocated buffers
        self.message_data_bytes.clear();
        self.message_lengths.clear();
        self.message_offsets.clear();

        let mut current_offset = 0u32;

        // Prepare message data
        for msg in messages {
            let msg_bytes = msg.as_bytes();
            self.message_offsets.push(current_offset);
            self.message_lengths.push(msg_bytes.len() as u32);
            self.message_data_bytes.extend_from_slice(msg_bytes);
            current_offset += msg_bytes.len() as u32;
        }

        // Pad arrays to BATCH_SIZE
        while self.message_lengths.len() < BATCH_SIZE {
            self.message_offsets.push(current_offset);
            self.message_lengths.push(0);
        }

        // Pack bytes into u32 words
        let aligned_size = (self.message_data_bytes.len() + 3) & !3;
        self.message_data_bytes.resize(aligned_size, 0);
        let messages_u32: &[u32] = bytemuck::cast_slice(&self.message_data_bytes);
        let messages_bytes = bytemuck::cast_slice(messages_u32);

        // Write data to GPU buffers
        if !messages_bytes.is_empty() {
            self.queue
                .write_buffer(&buffer_set.messages_buffer, 0, messages_bytes);
        } else {
            self.queue
                .write_buffer(&buffer_set.messages_buffer, 0, &[0u8; 4]);
        }

        let lengths_bytes = bytemuck::cast_slice(&self.message_lengths);
        let offsets_bytes = bytemuck::cast_slice(&self.message_offsets);
        self.queue
            .write_buffer(&buffer_set.lengths_buffer, 0, lengths_bytes);
        self.queue
            .write_buffer(&buffer_set.offsets_buffer, 0, offsets_bytes);
        self.queue.write_buffer(&self.target_buffer, 0, target_hash);
        self.queue
            .write_buffer(&buffer_set.result_buffer, 0, &(-1i32).to_le_bytes());
        self.queue.write_buffer(
            &self.message_count_buffer,
            0,
            &(messages.len() as u32).to_le_bytes(),
        );
    }

    /// Submit batch to GPU (non-blocking)
    fn submit_batch(&mut self, use_set_b: bool, batch_size: usize) {
        let buffer_set = if use_set_b {
            &self.buffer_set_b
        } else {
            &self.buffer_set_a
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("MD5 Command Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("MD5 Crack Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &buffer_set.bind_group, &[]);

            let num_workgroups = (batch_size as u32).div_ceil(64);
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        // Copy result to staging buffer
        encoder.copy_buffer_to_buffer(
            &buffer_set.result_buffer,
            0,
            &buffer_set.staging_buffer,
            0,
            4,
        );

        // Submit commands (non-blocking)
        self.queue.submit(Some(encoder.finish()));
    }

    /// Read result from staging buffer (blocks until ready)
    fn read_result(&self, use_set_b: bool) -> Option<usize> {
        let buffer_set = if use_set_b {
            &self.buffer_set_b
        } else {
            &self.buffer_set_a
        };

        let buffer_slice = buffer_set.staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        self.device
            .poll(wgpu::PollType::Wait)
            .expect("Failed to poll device");
        receiver.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();
        let result: i32 = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        drop(data);
        buffer_set.staging_buffer.unmap();

        if result >= 0 {
            Some(result as usize)
        } else {
            None
        }
    }

    /// Process a batch with GPU timing information (for benchmarking)
    /// Returns (result_index, gpu_time_ns) where gpu_time_ns is the GPU execution time in nanoseconds
    pub fn process_batch_with_timing(
        &mut self,
        messages: &[&str],
        target_hash: &[u8; 16],
    ) -> (Option<usize>, Option<u64>) {
        if !self.supports_timestamps {
            // Fall back to regular processing without timing
            return (self.process_batch(messages, target_hash), None);
        }

        // Reuse pre-allocated buffers
        self.message_data_bytes.clear();
        self.message_lengths.clear();
        self.message_offsets.clear();

        let mut current_offset = 0u32;

        // Prepare message data
        for msg in messages {
            let msg_bytes = msg.as_bytes();
            self.message_offsets.push(current_offset);
            self.message_lengths.push(msg_bytes.len() as u32);
            self.message_data_bytes.extend_from_slice(msg_bytes);
            current_offset += msg_bytes.len() as u32;
        }

        // Pad arrays to BATCH_SIZE
        while self.message_lengths.len() < BATCH_SIZE {
            self.message_offsets.push(current_offset);
            self.message_lengths.push(0);
        }

        // Pack bytes into u32 words with bytemuck (zero-copy)
        let aligned_size = (self.message_data_bytes.len() + 3) & !3;
        self.message_data_bytes.resize(aligned_size, 0);
        let messages_u32: &[u32] = bytemuck::cast_slice(&self.message_data_bytes);
        let messages_bytes = bytemuck::cast_slice(messages_u32);

        // Use buffer_set_a for timing measurements
        let buffer_set = &self.buffer_set_a;

        if !messages_bytes.is_empty() {
            self.queue
                .write_buffer(&buffer_set.messages_buffer, 0, messages_bytes);
        } else {
            self.queue
                .write_buffer(&buffer_set.messages_buffer, 0, &[0u8; 4]);
        }

        let lengths_bytes = bytemuck::cast_slice(&self.message_lengths);
        let offsets_bytes = bytemuck::cast_slice(&self.message_offsets);
        self.queue
            .write_buffer(&buffer_set.lengths_buffer, 0, lengths_bytes);
        self.queue
            .write_buffer(&buffer_set.offsets_buffer, 0, offsets_bytes);
        self.queue.write_buffer(&self.target_buffer, 0, target_hash);
        self.queue
            .write_buffer(&buffer_set.result_buffer, 0, &(-1i32).to_le_bytes());
        self.queue.write_buffer(
            &self.message_count_buffer,
            0,
            &(messages.len() as u32).to_le_bytes(),
        );

        // Create timestamp query set
        let query_set = self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Timestamp Query Set"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });

        let query_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Query Resolve Buffer"),
            size: 16, // 2 timestamps * 8 bytes
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let query_staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Query Staging Buffer"),
            size: 16,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create command encoder and dispatch with timestamps
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("MD5 Command Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("MD5 Crack Pass"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set: &query_set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &buffer_set.bind_group, &[]);

            let num_workgroups = (messages.len() as u32).div_ceil(64);
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        // Resolve timestamp queries
        encoder.resolve_query_set(&query_set, 0..2, &query_buffer, 0);
        encoder.copy_buffer_to_buffer(&query_buffer, 0, &query_staging_buffer, 0, 16);

        // Copy result to staging buffer
        encoder.copy_buffer_to_buffer(
            &buffer_set.result_buffer,
            0,
            &buffer_set.staging_buffer,
            0,
            4,
        );

        // Submit commands
        self.queue.submit(Some(encoder.finish()));

        // Read result
        let buffer_slice = buffer_set.staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        self.device
            .poll(wgpu::PollType::Wait)
            .expect("Failed to poll device");
        receiver.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();
        let result: i32 = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        drop(data);
        buffer_set.staging_buffer.unmap();

        // Read timestamps
        let query_slice = query_staging_buffer.slice(..);
        let (sender2, receiver2) = std::sync::mpsc::channel();
        query_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender2.send(result).unwrap();
        });

        self.device
            .poll(wgpu::PollType::Wait)
            .expect("Failed to poll device");
        receiver2.recv().unwrap().unwrap();

        let timestamp_data = query_slice.get_mapped_range();
        let start_timestamp = u64::from_le_bytes([
            timestamp_data[0],
            timestamp_data[1],
            timestamp_data[2],
            timestamp_data[3],
            timestamp_data[4],
            timestamp_data[5],
            timestamp_data[6],
            timestamp_data[7],
        ]);
        let end_timestamp = u64::from_le_bytes([
            timestamp_data[8],
            timestamp_data[9],
            timestamp_data[10],
            timestamp_data[11],
            timestamp_data[12],
            timestamp_data[13],
            timestamp_data[14],
            timestamp_data[15],
        ]);
        drop(timestamp_data);
        query_staging_buffer.unmap();

        // Calculate elapsed time in nanoseconds
        let timestamp_period = self.queue.get_timestamp_period();
        let gpu_time_ns =
            ((end_timestamp - start_timestamp) as f64 * timestamp_period as f64) as u64;

        let result_idx = if result >= 0 {
            Some(result as usize)
        } else {
            None
        };

        (result_idx, Some(gpu_time_ns))
    }

    /// Get whether this GPU supports timestamp queries
    pub fn supports_timestamps(&self) -> bool {
        self.supports_timestamps
    }
}
