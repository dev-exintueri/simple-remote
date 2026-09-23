//! Spike only (throwaway): can this process create an H.264 encoder MFT and encode frames?
//!
//! Usage: mf_probe [--software]
//! - default: first hardware encoder MFT (async), D3D11 NV12 texture input via DXGI device manager.
//! - --software: first non-hardware encoder MFT (sync), system-memory NV12 input.
//! Prints the running user and session first so a SYSTEM run (PsExec -s -i) can be told apart.
//! Last line: RESULT mode=.. encoder=".." frames_in=.. frames_out=.. bytes_out=.. first_output_ms=..

use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};

use windows::core::{Interface, Result, GUID, PWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D10::ID3D10Multithread;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFAttributes, IMFDXGIDeviceManager, IMFMediaEventGenerator, IMFSample,
    IMFTransform, MFCreateDXGIDeviceManager, MFCreateDXGISurfaceBuffer, MFCreateMediaType,
    MFCreateMemoryBuffer, MFCreateSample, MFMediaType_Video, MFStartup, MFTEnumEx,
    MFVideoFormat_H264, MFVideoFormat_NV12, MFVideoInterlace_Progressive, METransformHaveOutput,
    METransformNeedInput, MFSTARTUP_FULL, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE,
    MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_HARDWARE_URL_Attribute, MFT_FRIENDLY_NAME_Attribute,
    MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_START_OF_STREAM,
    MFT_MESSAGE_SET_D3D_MANAGER, MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES,
    MFT_OUTPUT_STREAM_PROVIDES_SAMPLES, MFT_REGISTER_TYPE_INFO, MF_EVENT_FLAG_NO_WAIT,
    MF_E_TRANSFORM_NEED_MORE_INPUT, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_TRANSFORM_ASYNC_UNLOCK, MF_VERSION,
};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_MULTITHREADED};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::System::WindowsProgramming::GetUserNameW;

const W: u32 = 1920;
const H: u32 = 1080;
const FRAMES: u32 = 30;
const FRAME_100NS: i64 = 10_000_000 / 30;
const TIMEOUT: Duration = Duration::from_secs(5);

fn current_user() -> String {
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    unsafe {
        match GetUserNameW(Some(PWSTR(buf.as_mut_ptr())), &mut len) {
            Ok(()) => String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]),
            Err(e) => format!("<err {e}>"),
        }
    }
}

fn current_session() -> u32 {
    let mut sid = u32::MAX;
    unsafe {
        let _ = ProcessIdToSessionId(GetCurrentProcessId(), &mut sid);
    }
    sid
}

unsafe fn allocated_string(act: &IMFActivate, key: &GUID) -> Option<String> {
    let mut p = PWSTR::null();
    let mut len = 0u32;
    unsafe {
        act.GetAllocatedString(key, &mut p, &mut len).ok()?;
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p.0, len as usize));
        CoTaskMemFree(Some(p.0 as *const _));
        Some(s)
    }
}

/// All H.264 encoder activates, hardware ones first when `hardware_only` is false.
unsafe fn enum_h264(hardware_only: bool) -> Result<Vec<IMFActivate>> {
    let out_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };
    let mut flags = MFT_ENUM_FLAG_SORTANDFILTER;
    if hardware_only {
        flags |= MFT_ENUM_FLAG_HARDWARE;
    }
    let mut arr: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count = 0u32;
    unsafe {
        MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, flags, None, Some(&out_info), &mut arr, &mut count)?;
        let list = if arr.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(arr, count as usize).iter().flatten().cloned().collect()
        };
        if !arr.is_null() {
            CoTaskMemFree(Some(arr as *const _));
        }
        Ok(list)
    }
}

fn pack_u64(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

fn step(label: &str, r: Result<()>) -> Result<()> {
    match &r {
        Ok(()) => println!("  {label}: OK"),
        Err(e) => println!("  {label}: HRESULT {:#010x} ({})", e.code().0, e.message()),
    }
    r
}

unsafe fn set_types(t: &IMFTransform) -> Result<()> {
    unsafe {
        let out_type = MFCreateMediaType()?;
        out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        out_type.SetUINT32(&MF_MT_AVG_BITRATE, 8_000_000)?;
        out_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        out_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(W, H))?;
        out_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(30, 1))?;
        // Encoders need the output type before the input type.
        step("SetOutputType(H264 1920x1080 30fps)", t.SetOutputType(0, &out_type, 0))?;

        let in_type = MFCreateMediaType()?;
        in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        in_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        in_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(W, H))?;
        in_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(30, 1))?;
        step("SetInputType(NV12)", t.SetInputType(0, &in_type, 0))
    }
}

struct Counters {
    frames_in: u32,
    frames_out: u32,
    bytes_out: u64,
    first_output: Option<Duration>,
    started: Instant,
}

/// Pull one output. Ok(false) means the MFT needs more input.
unsafe fn pull_output(t: &IMFTransform, provides_samples: bool, out_size: u32, c: &mut Counters) -> Result<bool> {
    unsafe {
        let own_sample: Option<IMFSample> = if provides_samples {
            None
        } else {
            let s = MFCreateSample()?;
            s.AddBuffer(&MFCreateMemoryBuffer(out_size.max(W * H))?)?;
            Some(s)
        };
        let mut bufs = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: ManuallyDrop::new(own_sample),
            dwStatus: 0,
            pEvents: ManuallyDrop::new(None),
        }];
        let mut status = 0u32;
        let r = t.ProcessOutput(0, &mut bufs, &mut status);
        let sample = ManuallyDrop::take(&mut bufs[0].pSample);
        drop(ManuallyDrop::take(&mut bufs[0].pEvents));
        match r {
            Ok(()) => {
                if let Some(s) = sample {
                    c.frames_out += 1;
                    c.bytes_out += s.GetTotalLength()? as u64;
                    c.first_output.get_or_insert(c.started.elapsed());
                }
                Ok(true)
            }
            Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(false),
            Err(e) => Err(e),
        }
    }
}

unsafe fn output_info(t: &IMFTransform) -> Result<(bool, u32)> {
    unsafe {
        let info = t.GetOutputStreamInfo(0)?;
        let provides = info.dwFlags
            & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0 as u32)
            != 0;
        println!("  output stream: provides_samples={provides} cbSize={}", info.cbSize);
        Ok((provides, info.cbSize))
    }
}

unsafe fn begin_streaming(t: &IMFTransform) -> Result<()> {
    unsafe {
        step("NOTIFY_BEGIN_STREAMING", t.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0))?;
        step("NOTIFY_START_OF_STREAM", t.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0))
    }
}

/// Hardware path: async MFT, D3D11 NV12 texture input.
unsafe fn run_hardware(act: &IMFActivate, c: &mut Counters) -> Result<()> {
    unsafe {
        let attrs = act.cast::<IMFAttributes>()?;
        let _ = step("MF_TRANSFORM_ASYNC_UNLOCK", attrs.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1));
        let t: IMFTransform = act.ActivateObject()?;
        // Unlock again on the transform's own attribute store (async MFTs check it there).
        if let Ok(ta) = t.GetAttributes() {
            let _ = step("transform MF_TRANSFORM_ASYNC_UNLOCK", ta.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1));
        }

        let mut device: Option<ID3D11Device> = None;
        step(
            "D3D11CreateDevice(VIDEO|BGRA)",
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            ),
        )?;
        let device = device.expect("device");
        let _ = device.cast::<ID3D10Multithread>()?.SetMultithreadProtected(true);

        let mut token = 0u32;
        let mut manager: Option<IMFDXGIDeviceManager> = None;
        step("MFCreateDXGIDeviceManager", MFCreateDXGIDeviceManager(&mut token, &mut manager))?;
        let manager = manager.expect("manager");
        step("ResetDevice", manager.ResetDevice(&device, token))?;
        step("SET_D3D_MANAGER", t.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, manager.as_raw() as usize))?;

        set_types(&t)?;
        let (provides, out_size) = output_info(&t)?;

        let mut desc = D3D11_TEXTURE2D_DESC {
            Width: W,
            Height: H,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_NV12,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut tex: Option<ID3D11Texture2D> = None;
        if step("CreateTexture2D(NV12, RENDER_TARGET)", device.CreateTexture2D(&desc, None, Some(&mut tex))).is_err() {
            desc.BindFlags = 0;
            step("CreateTexture2D(NV12, no bind flags)", device.CreateTexture2D(&desc, None, Some(&mut tex)))?;
        }
        let tex = tex.expect("texture");

        let events = t.cast::<IMFMediaEventGenerator>()?;
        begin_streaming(&t)?;
        c.started = Instant::now();
        while c.frames_out < FRAMES && c.started.elapsed() < TIMEOUT {
            let ev = match events.GetEvent(MF_EVENT_FLAG_NO_WAIT) {
                Ok(ev) => ev,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
            };
            let kind = ev.GetType()?;
            if kind == METransformNeedInput.0 as u32 && c.frames_in < FRAMES {
                let buffer = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &tex, 0, false)?;
                let sample = MFCreateSample()?;
                sample.AddBuffer(&buffer)?;
                sample.SetSampleTime(c.frames_in as i64 * FRAME_100NS)?;
                sample.SetSampleDuration(FRAME_100NS)?;
                t.ProcessInput(0, &sample, 0)?;
                c.frames_in += 1;
            } else if kind == METransformHaveOutput.0 as u32 {
                pull_output(&t, provides, out_size, c)?;
            }
        }
        Ok(())
    }
}

/// Software path: sync MFT, system-memory NV12 input.
unsafe fn run_software(act: &IMFActivate, c: &mut Counters) -> Result<()> {
    unsafe {
        let t: IMFTransform = act.ActivateObject()?;
        set_types(&t)?;
        let (provides, out_size) = output_info(&t)?;
        begin_streaming(&t)?;
        let frame_bytes = W * H * 3 / 2;
        c.started = Instant::now();
        while c.frames_in < FRAMES && c.started.elapsed() < TIMEOUT {
            let buffer = MFCreateMemoryBuffer(frame_bytes)?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None)?;
            // Mid-grey frame with a moving bright band so frames differ.
            let data = std::slice::from_raw_parts_mut(ptr, frame_bytes as usize);
            data.fill(128);
            let band = (c.frames_in * 32 % H) as usize;
            data[band * W as usize..(band + 16) * W as usize].fill(235);
            buffer.Unlock()?;
            buffer.SetCurrentLength(frame_bytes)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(c.frames_in as i64 * FRAME_100NS)?;
            sample.SetSampleDuration(FRAME_100NS)?;
            t.ProcessInput(0, &sample, 0)?;
            c.frames_in += 1;
            while pull_output(&t, provides, out_size, c)? {}
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let software = std::env::args().any(|a| a == "--software");
    println!("user={} session={}", current_user(), current_session());
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;

        let all = enum_h264(false)?;
        println!("H.264 encoders: {}", all.len());
        for a in &all {
            let name = allocated_string(a, &MFT_FRIENDLY_NAME_Attribute).unwrap_or_default();
            let hw = allocated_string(a, &MFT_ENUM_HARDWARE_URL_Attribute).is_some();
            println!("  - {name} (hardware={hw})");
        }

        let pick = if software {
            all.iter().find(|a| allocated_string(a, &MFT_ENUM_HARDWARE_URL_Attribute).is_none()).cloned()
        } else {
            enum_h264(true)?.into_iter().next()
        };
        let mode = if software { "software" } else { "hardware" };
        let Some(act) = pick else {
            println!("RESULT mode={mode} encoder=\"<none>\" frames_in=0 frames_out=0 bytes_out=0 first_output_ms=-");
            return Ok(());
        };
        let name = allocated_string(&act, &MFT_FRIENDLY_NAME_Attribute).unwrap_or_default();
        println!("using: {name}");

        let mut c = Counters { frames_in: 0, frames_out: 0, bytes_out: 0, first_output: None, started: Instant::now() };
        let r = if software { run_software(&act, &mut c) } else { run_hardware(&act, &mut c) };
        if let Err(e) = &r {
            println!("  encode loop error: HRESULT {:#010x} ({})", e.code().0, e.message());
        }
        let first = c.first_output.map(|d| format!("{}", d.as_millis())).unwrap_or_else(|| "-".into());
        println!(
            "RESULT mode={mode} encoder=\"{name}\" frames_in={} frames_out={} bytes_out={} first_output_ms={first}",
            c.frames_in, c.frames_out, c.bytes_out
        );
    }
    Ok(())
}
