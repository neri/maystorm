//! Audio API

use crate::{
    sync::Mutex,
    task::scheduler::{Priority, SpawnOption, Timer},
    *,
};
use alloc::{
    boxed::Box,
    collections::BTreeMap,
    slice,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    cell::UnsafeCell,
    mem::transmute,
    mem::MaybeUninit,
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

pub type FreqType = f64;
pub type SampleType = f64;

pub const FREQ_MIN: FreqType = 20.0;
pub const FREQ_MAX: FreqType = 20_000.0;
pub const AUDIO_LEVEL_MAX: SampleType = 1.0;
pub const AUDIO_LEVEL_MIN: SampleType = -1.0;

static mut AUDIO_MANAGER: MaybeUninit<UnsafeCell<AudioManager>> = MaybeUninit::uninit();

pub struct AudioManager {
    audio_driver: Mutex<Option<Arc<dyn AudioDriver>>>,
    emitters: Mutex<BTreeMap<AudioContextHandle, AudioEmitter>>,
    contexts: Mutex<BTreeMap<AudioContextHandle, Weak<AudioContext>>>,
}

impl AudioManager {
    pub const DEFAULT_SAMPLE_RATE: FreqType = 44_100.0;

    #[inline]
    pub unsafe fn init() {
        check_once_call!();

        AUDIO_MANAGER = MaybeUninit::new(UnsafeCell::new(AudioManager::new()));

        SpawnOption::with_priority(Priority::High).start(Self::_audio_thread, 0, "Audio Manager");
    }

    #[inline]
    fn new() -> Self {
        Self {
            audio_driver: Mutex::new(None),
            emitters: Mutex::new(BTreeMap::new()),
            contexts: Mutex::new(BTreeMap::new()),
        }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*AUDIO_MANAGER.assume_init_ref().get() }
    }

    #[inline]
    fn next_handle() -> AudioContextHandle {
        static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(1);
        unsafe {
            AudioContextHandle(NonZeroUsize::new_unchecked(
                NEXT_HANDLE.fetch_add(1, Ordering::SeqCst),
            ))
        }
    }

    #[inline]
    pub fn register_context(handle: AudioContextHandle, ctx: Weak<AudioContext>) {
        let mut contexts = Self::shared().contexts.lock().unwrap();
        contexts.insert(handle, ctx);
    }

    #[inline]
    pub fn unregister_context(handle: AudioContextHandle) {
        let mut contexts = Self::shared().contexts.lock().unwrap();
        contexts.remove(&handle);
    }

    #[inline]
    pub unsafe fn set_audio_driver(destination: Arc<dyn AudioDriver>) {
        *Self::shared().audio_driver.lock().unwrap() = Some(destination);
    }

    pub fn master_gain() -> SampleType {
        0.1
    }

    #[inline]
    pub fn reinterpret_i16(src: SampleType) -> i16 {
        (src * i16::MAX as SampleType) as i16
    }

    #[inline]
    pub fn schedule_emitter(emitter: AudioEmitter) {
        let shared = Self::shared();
        let handle = emitter.handle;
        shared.emitters.lock().unwrap().insert(handle, emitter);
    }

    #[inline]
    pub fn remove_emitter(handle: AudioContextHandle) {
        let shared = Self::shared();
        let _ = shared.emitters.lock().unwrap().remove(&handle);
    }

    /// Audio Scheduler
    fn _audio_thread(_: usize) {
        let shared = Self::shared();

        let driver = loop {
            Timer::sleep(Duration::from_millis(100));
            match Self::shared().audio_driver.lock().unwrap().clone() {
                Some(v) => break v,
                None => (),
            }
        };
        driver.set_master_volume(10);

        let buffer_len = driver.size_of_buffer();
        let mut buffer = Vec::with_capacity(buffer_len);
        buffer.resize(buffer_len, 0);

        let mut buffer_mute = Vec::with_capacity(buffer_len);
        buffer_mute.resize(buffer_len, 0);

        let wave_len = buffer_len / 4;
        let wave_buffer =
            unsafe { slice::from_raw_parts_mut(transmute(buffer.get_unchecked_mut(0)), wave_len) };

        let timer_len = (wave_len as f64 / Self::DEFAULT_SAMPLE_RATE * 1000.0) as u64 - 1;

        // panic!("LEN {} {}", wave_len, timer_len);

        loop {
            let mut emitters = shared.emitters.lock().unwrap();
            let is_mute = if emitters.len() > 0 {
                let master_gain = Self::master_gain();
                for data in wave_buffer.iter_mut() {
                    let mut sum = 0.0;
                    for emitter in emitters.values_mut() {
                        sum += emitter.render(master_gain);
                    }
                    *data = (Self::reinterpret_i16(sum) as u32) * 0x0001_0001;
                }
                false
            } else {
                true
            };
            drop(emitters);
            if is_mute {
                loop {
                    if driver.write_block(buffer_mute.as_slice()).is_some() {
                        break;
                    }
                    Timer::sleep(Duration::from_millis(1));
                }
            } else {
                loop {
                    if driver.write_block(buffer.as_slice()).is_some() {
                        break;
                    }
                    Timer::sleep(Duration::from_millis(1));
                }
            }
            Timer::sleep(Duration::from_millis(timer_len));
        }
    }
}

pub trait AudioDriver {
    /// Sets the master volume.
    ///
    /// # Arguments
    ///
    /// * `gain` - Specifies the gain in dB. 0 indicates maximum volume and 1 indicates -1 dB.
    ///
    /// # Results
    ///
    /// Returns `true` when the volume is at the lowest level supported by the hardware.
    fn set_master_volume(&self, gain: usize) -> bool;

    fn size_of_buffer(&self) -> usize;

    fn write_block(&self, data: &[u8]) -> Option<()>;
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AudioContextHandle(NonZeroUsize);

pub struct AudioEmitter {
    handle: AudioContextHandle,
    filters: Vec<Box<dyn AudioNodeFilter>>,
}

impl AudioEmitter {
    #[inline]
    pub fn new(filters: Vec<Box<dyn AudioNodeFilter>>) -> Self {
        Self {
            handle: AudioManager::next_handle(),
            filters,
        }
    }

    #[inline]
    pub fn render(&mut self, master_data: SampleType) -> SampleType {
        let mut acc = master_data;
        for filter in self.filters.iter_mut() {
            acc = (filter)(acc);
        }
        acc
    }

    #[inline]
    pub const fn handle(&self) -> AudioContextHandle {
        self.handle
    }
}

pub struct AudioContext {
    handle: AudioContextHandle,
    handles: Mutex<Vec<AudioContextHandle>>,
}

impl AudioContext {
    #[inline]
    pub fn new() -> Arc<Self> {
        let handle = AudioManager::next_handle();
        let ctx = Arc::new(Self {
            handle,
            handles: Mutex::new(Vec::new()),
        });
        AudioManager::register_context(handle, Arc::downgrade(&ctx));
        ctx
    }

    #[inline]
    pub fn create_oscillator(
        self: &Arc<Self>,
        freq: FreqType,
        osc_type: OscType,
    ) -> Box<AudioNode> {
        let ctx = Arc::downgrade(self);
        let length = AudioManager::DEFAULT_SAMPLE_RATE / freq;
        match osc_type {
            OscType::Sine => SineWaveOscillator::new(ctx, length),
            OscType::Square => PulseWaveOscillator::new(ctx, length, 0.5),
            OscType::Pulse(duty_cycles) => PulseWaveOscillator::new(ctx, length, duty_cycles),
            OscType::Sawtooth => SawtoothWaveOscillator::new(ctx, length),
            OscType::Triangle => TriangleWaveOscillator::new(ctx, length),
        }
    }

    #[inline]
    pub fn create_gain(self: &Arc<Self>, gain: SampleType) -> Box<AudioNode> {
        let ctx = Arc::downgrade(self);
        AudioNode::new(ctx, move |v| v * gain)
    }

    #[inline]
    pub fn destination(self: &Arc<Self>) -> Box<AudioNode> {
        AudioNode::closed(Arc::downgrade(self))
    }

    pub fn schedule(self: &Arc<Self>, emitter: AudioEmitter) -> NoteControl {
        let handle = emitter.handle();
        self.handles.lock().unwrap().push(handle);
        AudioManager::schedule_emitter(emitter);
        NoteControl { handle }
    }
}

impl Drop for AudioContext {
    fn drop(&mut self) {
        AudioManager::unregister_context(self.handle);

        let vec = self.handles.lock().unwrap();
        for handle in vec.iter() {
            AudioManager::remove_emitter(*handle);
        }
    }
}

/// Oscillator Type
#[derive(Debug, Clone, Copy)]
pub enum OscType {
    Sine,
    Square,
    Pulse(f64),
    Sawtooth,
    Triangle,
}

pub trait AudioNodeFilter = FnMut(SampleType) -> SampleType + 'static;

pub struct AudioNode {
    ctx: Weak<AudioContext>,
    filter: Box<dyn AudioNodeFilter>,
    destination: Option<Box<AudioNode>>,
    is_closed: bool,
}

impl AudioNode {
    #[inline]
    pub fn new<F>(ctx: Weak<AudioContext>, filter: F) -> Box<Self>
    where
        F: AudioNodeFilter,
    {
        Box::new(Self {
            ctx,
            filter: Box::new(filter),
            destination: None,
            is_closed: false,
        })
    }

    #[inline]
    pub fn closed(ctx: Weak<AudioContext>) -> Box<Self> {
        Box::new(Self {
            ctx,
            filter: Box::new(|_| SampleType::NAN),
            destination: None,
            is_closed: true,
        })
    }

    #[inline]
    pub const fn number_of_outputs(&self) -> usize {
        self.destination.is_some() as usize
    }

    #[inline]
    pub const fn is_leaf(&self) -> bool {
        self.number_of_outputs() == 0
    }

    #[inline]
    pub const fn is_closed(&self) -> bool {
        self.is_closed
    }

    #[inline]
    pub fn connect(&mut self, destination: Box<AudioNode>) {
        if !self.is_closed() {
            self.destination = Some(destination);
        }
    }

    #[inline]
    pub fn into_filter(self) -> Box<dyn AudioNodeFilter> {
        self.filter
    }

    pub fn final_destination(&self) -> Option<&Box<AudioNode>> {
        if let Some(destination) = self.destination.as_ref() {
            if destination.is_leaf() {
                self.destination.as_ref()
            } else {
                destination.final_destination()
            }
        } else {
            None
        }
    }

    pub fn take_final_destination(&mut self) -> Option<Box<AudioNode>> {
        if let Some(destination) = self.destination.as_mut() {
            if destination.is_leaf() {
                self.destination.take()
            } else {
                destination.take_final_destination()
            }
        } else {
            None
        }
    }

    pub fn start(mut self) -> Result<NoteControl, Self> {
        // The final destination must exist and be closed
        if let Some(leaf) = self.final_destination() {
            if !leaf.is_closed() {
                return Err(self);
            }
        } else if !self.is_closed() {
            return Err(self);
        }
        let ctx = match self.ctx.upgrade() {
            Some(v) => v,
            None => return Err(self),
        };

        let _ = self.take_final_destination();
        let mut vec = Vec::new();
        while let Some(leaf) = self.take_final_destination() {
            vec.push(leaf);
        }
        vec.push(Box::new(self));
        let filters: Vec<Box<dyn AudioNodeFilter>> =
            vec.into_iter().rev().map(|v| v.into_filter()).collect();

        Ok(ctx.schedule(AudioEmitter::new(filters)))
    }
}

pub struct NoteControl {
    handle: AudioContextHandle,
}

impl NoteControl {
    #[inline]
    pub fn stop(&self) {
        AudioManager::remove_emitter(self.handle);
    }
}

pub struct PulseWaveOscillator {
    full_length: f64,
    pos_length: f64,
    time: f64,
}

impl PulseWaveOscillator {
    pub fn new(ctx: Weak<AudioContext>, length: f64, duty_cycles: f64) -> Box<AudioNode> {
        let mut this = Self {
            full_length: length,
            pos_length: length * duty_cycles,
            time: 0.0,
        };
        AudioNode::new(ctx, move |data: SampleType| {
            let result = if this.time < this.pos_length {
                data
            } else {
                -data
            };
            this.time = this.time + 1.0;
            if this.time >= this.full_length {
                this.time -= this.full_length;
            }
            result
        })
    }
}

pub struct SineWaveOscillator {
    length: f64,
    delta: f64,
    time: f64,
}

impl SineWaveOscillator {
    pub fn new(ctx: Weak<AudioContext>, length: f64) -> Box<AudioNode> {
        let mut this = Self {
            length,
            delta: core::f64::consts::PI * 2.0 / length,
            time: 0.0,
        };
        AudioNode::new(ctx, move |data| {
            let result = data * libm::sin(this.delta * this.time);
            this.time = this.time + 1.0;
            if this.time >= this.length {
                this.time -= this.length;
            }
            result
        })
    }
}

/// TODO:
pub struct SawtoothWaveOscillator {
    length: f64,
    delta: f64,
    time: f64,
}

impl SawtoothWaveOscillator {
    pub fn new(ctx: Weak<AudioContext>, length: f64) -> Box<AudioNode> {
        let mut this = Self {
            length,
            delta: 2.0 / length,
            time: 0.0,
        };
        AudioNode::new(ctx, move |data: SampleType| {
            let result = data * (this.time * this.delta - 1.0);
            this.time = this.time + 1.0;
            if this.time >= this.length {
                this.time -= this.length;
            }
            result
        })
    }
}

/// TODO:
pub struct TriangleWaveOscillator {}

impl TriangleWaveOscillator {
    pub fn new(ctx: Weak<AudioContext>, _length: f64) -> Box<AudioNode> {
        AudioNode::new(ctx, move |data: SampleType| data)
    }
}

/// Experimental Envelope Generator
pub struct NoteOnParams {
    atack: f64,
    decay: f64,
    sustain: f64,
    atack_delta: f64,
    decay_delta: f64,
    current_gain: f64,
    time: f64,
}

impl NoteOnParams {
    pub fn new(ctx: Weak<AudioContext>, atack: f64, decay: f64, sustain: f64) -> Box<AudioNode> {
        let mut this = Self {
            atack,
            decay: atack + decay,
            sustain,
            atack_delta: 1.0 / atack,
            decay_delta: (1.0 - sustain) / decay,
            current_gain: if atack < 1.0 { 1.0 } else { 0.0 },
            time: 0.0,
        };
        AudioNode::new(ctx, move |data| {
            if this.time < this.atack {
                this.current_gain += this.atack_delta;
            } else if this.time < this.decay {
                this.current_gain -= this.decay_delta;
            } else {
                return data * this.sustain;
            }
            this.time = this.time + 1.0;
            data * this.current_gain
        })
    }
}
