use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;
use wgpu::util::DeviceExt;

// How many hashes do we compute at a time?
pub const BATCH_SIZE: usize = 4096;
pub const MAX_MSG_SIZE: usize = 256;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct TargetHash {
    pub data: [u32; 4],
}

/// GPU-based MD5 hash cracker
pub struct GpuCracker {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuCracker {
    /// Initialize the GPU cracker
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create wgpu instance with Vulkan backend (for AMD GPU support)
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
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
            .await
            .ok_or("Failed to find suitable GPU adapter")?;

        println!("Using GPU: {}", adapter.get_info().name);

        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
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
            entry_point: "md5_crack",
            compilation_options: Default::default(),
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
        })
    }

    /// Process a batch of messages and check against target hash
    pub fn process_batch(&self, messages: &[&str], target_hash: &[u8; 16]) -> Option<usize> {
        // Prepare message data
        let mut message_data_bytes = Vec::new();
        let mut message_lengths = Vec::new();
        let mut message_offsets = Vec::new();
        let mut current_offset = 0u32;

        for msg in messages {
            let msg_bytes = msg.as_bytes();
            message_offsets.push(current_offset);
            message_lengths.push(msg_bytes.len() as u32);
            message_data_bytes.extend_from_slice(msg_bytes);
            current_offset += msg_bytes.len() as u32;
        }

        // Convert byte data to u32 array (pack 4 bytes per u32, little-endian)
        let mut message_data_u32 = Vec::new();
        for chunk in message_data_bytes.chunks(4) {
            let mut word = 0u32;
            for (i, &byte) in chunk.iter().enumerate() {
                word |= (byte as u32) << (i * 8);
            }
            message_data_u32.push(word);
        }

        // Pad arrays to BATCH_SIZE
        while message_lengths.len() < BATCH_SIZE {
            message_offsets.push(current_offset);
            message_lengths.push(0);
        }

        // Convert target hash to u32 array (little-endian)
        let target = TargetHash {
            data: [
                u32::from_le_bytes([
                    target_hash[0],
                    target_hash[1],
                    target_hash[2],
                    target_hash[3],
                ]),
                u32::from_le_bytes([
                    target_hash[4],
                    target_hash[5],
                    target_hash[6],
                    target_hash[7],
                ]),
                u32::from_le_bytes([
                    target_hash[8],
                    target_hash[9],
                    target_hash[10],
                    target_hash[11],
                ]),
                u32::from_le_bytes([
                    target_hash[12],
                    target_hash[13],
                    target_hash[14],
                    target_hash[15],
                ]),
            ],
        };

        // Create GPU buffers - manually convert to bytes to avoid alignment issues
        let mut messages_bytes = Vec::with_capacity(message_data_u32.len() * 4);
        for &word in &message_data_u32 {
            messages_bytes.extend_from_slice(&word.to_le_bytes());
        }
        // Ensure buffer is not empty (required by wgpu)
        if messages_bytes.is_empty() {
            messages_bytes.push(0);
        }
        let messages_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Messages Buffer"),
                contents: &messages_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });

        let mut lengths_bytes = Vec::with_capacity(message_lengths.len() * 4);
        for &len in &message_lengths {
            lengths_bytes.extend_from_slice(&len.to_le_bytes());
        }
        let lengths_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Lengths Buffer"),
                contents: &lengths_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });

        let mut offsets_bytes = Vec::with_capacity(message_offsets.len() * 4);
        for &offset in &message_offsets {
            offsets_bytes.extend_from_slice(&offset.to_le_bytes());
        }
        let offsets_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Offsets Buffer"),
                contents: &offsets_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });

        let mut target_bytes = Vec::with_capacity(16);
        for &word in &target.data {
            target_bytes.extend_from_slice(&word.to_le_bytes());
        }
        let target_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Target Buffer"),
                contents: &target_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });

        let result_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Result Buffer"),
                contents: &(-1i32).to_le_bytes(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });

        let message_count_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Message Count Buffer"),
                    contents: &(messages.len() as u32).to_le_bytes(),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

        // Create staging buffer for reading results
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MD5 Bind Group"),
            layout: &self.bind_group_layout,
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
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch with workgroups based on actual batch size (each workgroup has 64 threads)
            let num_workgroups = (messages.len() as u32).div_ceil(64);
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        // Copy result to staging buffer
        encoder.copy_buffer_to_buffer(&result_buffer, 0, &staging_buffer, 0, 4);

        // Submit commands
        self.queue.submit(Some(encoder.finish()));

        // Read result
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();
        let result: i32 = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        drop(data);
        staging_buffer.unmap();

        if result >= 0 {
            Some(result as usize)
        } else {
            None
        }
    }

    /// Crack a hash using a wordlist
    pub fn crack(&self, target_hash: &[u8; 16], wordlist: &[&str]) -> Option<String> {
        for chunk in wordlist.chunks(BATCH_SIZE) {
            if let Some(idx) = self.process_batch(chunk, target_hash) {
                return Some(chunk[idx].to_string());
            }
        }
        None
    }
}
