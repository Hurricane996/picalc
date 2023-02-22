use std::{env::args, mem::size_of};

use wgpu::{
    include_wgsl, util::DeviceExt, BindGroup, BindGroupLayout, Buffer, BufferDescriptor,
    BufferUsages, CommandEncoder, ComputePass, Device, InstanceDescriptor, MapMode,
};

fn main() {
    pollster::block_on(run());
}

struct Square {
    bind_group: BindGroup,
    read_buffer: Buffer,
    storage_buffer: Buffer,
    _offset_buffer: Buffer,

    squares_per: u32,
    size: u32,
}
struct SquareCommonOptions<'a> {
    device: &'a Device,
    squares_per: u32,
    size: u32,

    storage_buffer_descriptor: &'a BufferDescriptor<'a>,
    read_buffer_descriptor: &'a BufferDescriptor<'a>,

    bind_group_layout: &'a BindGroupLayout,
}
impl Square {
    fn new(offset: [u32; 2], common_opts: &SquareCommonOptions) -> Self {
        let _offset_buffer =
            common_opts
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Bottom Right Offset Buffer"),
                    contents: bytemuck::cast_slice(&offset),
                    usage: BufferUsages::UNIFORM,
                });
        let storage_buffer = common_opts
            .device
            .create_buffer(common_opts.storage_buffer_descriptor);
        let read_buffer = common_opts
            .device
            .create_buffer(common_opts.read_buffer_descriptor);

        let bind_group = common_opts
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: common_opts.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: storage_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: _offset_buffer.as_entire_binding(),
                    },
                ],
            });

        Self {
            _offset_buffer,
            storage_buffer,
            read_buffer,
            bind_group,
            squares_per: common_opts.squares_per,
            size: common_opts.size,
        }
    }

    fn compute<'a>(&'a self, cpass: &mut ComputePass<'a>) {
        cpass.set_bind_group(1, &self.bind_group, &[]);
        cpass.dispatch_workgroups(
            self.size / (self.squares_per * 16),
            self.size / (self.squares_per * 16),
            1,
        );
    }

    fn copy(&self, encoder: &mut CommandEncoder) {
        encoder.copy_buffer_to_buffer(
            &self.storage_buffer,
            0,
            &self.read_buffer,
            0,
            (self.size / self.squares_per * self.size / self.squares_per
                * (std::mem::size_of::<u32>() as u32))
                .try_into()
                .unwrap(),
        );
    }

    fn map(&self) {
        self.read_buffer
            .slice(..)
            .map_async(MapMode::Read, move |e| {
                e.unwrap();
            });
    }

    fn get_total(&self) -> u32 {
        let data = self.read_buffer.slice(..).get_mapped_range();

        let data_u32: &[u32] = bytemuck::cast_slice(&data);

        data_u32.iter().sum()
    }

    #[allow(dead_code)]
    fn print(&self) {
        let data = self.read_buffer.slice(..).get_mapped_range();

        let data_u32: &[u32] = bytemuck::cast_slice(&data);

        for y in 0..self.size / self.squares_per {
            for x in 0..self.size / self.squares_per {
                print!(
                    "{}",
                    data_u32[((y * self.size) / self.squares_per + x) as usize]
                );
            }
            println!();
        }
    }
}

async fn run() {
    let size = args()
        .nth(1)
        .map(|x| x.parse::<usize>().expect("Bad Arg Format"))
        .unwrap_or(1024);

    env_logger::init();

    let instance = wgpu::Instance::new(InstanceDescriptor::default());

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .unwrap();

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .unwrap();

    let shader = device.create_shader_module(include_wgsl!("compute.wgsl"));

    let storage_buffer_descriptor = BufferDescriptor {
        label: Some("Storage Buffer"),
        size: (size_of::<u32>() * size / 8 * size / 8) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    };

    let read_buffer_descriptor = BufferDescriptor {
        label: Some("Read Buffer"),
        size: (size_of::<u32>() * size / 8 * size / 8) as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    };

    let options_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Size Buffer"),
        contents: bytemuck::cast_slice(&[size as u32, (size / 8) as u32]),
        usage: BufferUsages::UNIFORM,
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: "main",
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &compute_pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: options_buffer.as_entire_binding(),
        }],
    });

    let offset_bind_group_layout = compute_pipeline.get_bind_group_layout(1);

    let common_opts = SquareCommonOptions {
        device: &device,
        squares_per: 8,
        size: size as u32,
        storage_buffer_descriptor: &storage_buffer_descriptor,
        read_buffer_descriptor: &read_buffer_descriptor,
        bind_group_layout: &offset_bind_group_layout,
    };

    let s = (size / 8) as u32;
    let squares = [
        [7, 0],
        [7, 1],
        [7, 2],
        [7, 3],
        [6, 3],
        [6, 4],
        [6, 5],
        [5, 5],
    ]
    .map(|x| Square::new([x[0] * s, x[1] * s], &common_opts));

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);

        for square in &squares {
            square.compute(&mut cpass);
        }
    }
    for square in &squares {
        square.copy(&mut encoder);
    }
    queue.submit(Some(encoder.finish()));
    for square in &squares {
        square.map();
    }
    instance.poll_all(true);
    println!("GPU Done!");

    let squares = squares.iter().map(|s| s.get_total()).collect::<Vec<_>>();

    let full = ((size / 8) * (size / 8)) as u32;

    #[rustfmt::skip]
    #[allow(clippy::identity_op)]
    let total =
        full       + full       + full       + full       + full       + full       + full       + squares[0] + 
        full       + full       + full       + full       + full       + full       + full       + squares[1] + 
        full       + full       + full       + full       + full       + full       + full       + squares[2] + 
        full       + full       + full       + full       + full       + full       + squares[4] + squares[3] +
        full       + full       + full       + full       + full       + full       + squares[5] + 0          + 
        full       + full       + full       + full       + full       + squares[7] + squares[6] + 0          + 
        full       + full       + full       + squares[4] + squares[5] + squares[6] + 0          + 0          +
        squares[0] + squares[1] + squares[2] + squares[3] + 0          + 0          + 0          + 0
    ;

    println!("pi = {}/{}", (total as u64) * 4, (size - 1) * (size - 1));
}
