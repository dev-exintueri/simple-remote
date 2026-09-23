# Host 플랫폼 조사: Windows / macOS 화면 캡처, 입력, 권한, DPI, 클립보드, 인코딩

- 조사일(자료 확인 날짜): 2026-09-23
- 대상: P2P 원격 제어 프로그램의 host 측 플랫폼 기술. Windows 우선, macOS 는 후속 단계 요약.

## 읽는 법

- 모든 주장 끝에 출처 URL 을 붙였다. Microsoft Learn, Apple Developer, 코덱 프로젝트 문서, 오픈소스 소스 코드(RustDesk, Sunshine, TigerVNC)가 1차 출처다.
- "미확인(2차 출처)": 1차 출처에서 찾지 못하고 블로그, 이슈, 포럼 등에서만 본 내용.
- "추론": 문서 문장 두 개 이상을 맞대어 이끌어 낸 결론. 문서에 그대로 적힌 것은 아니다.
- 최소 OS 버전은 문서에 적힌 경우 그대로 옮겼다.
- 설계 결정에 직접 쓰려면 11절(설계에 바로 영향을 주는 사실)과 12절(RustDesk 대조표)부터 보면 된다.

## 1. 화면 캡처

GitHub 링크는 조사일 HEAD commit 에 고정했다. RustDesk `58ff78a8`, Sunshine `c48e50e4`, TigerVNC `3dcfda19`, robmikh/Win32CaptureSample `49fefe79`, Windows-classic-samples `434f6002`.

### 1.1 DXGI Desktop Duplication API (DDA, `IDXGIOutputDuplication`)

- 최소 버전: Windows 8 / Server 2012. Windows 7 에 Platform Update 를 깔아도 `DuplicateOutput` 은 `E_NOTIMPL`. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput
- 픽셀 형식: `DuplicateOutput` 으로 받는 surface 는 display mode 와 상관없이 항상 `DXGI_FORMAT_B8G8R8A8_UNORM`. https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api
- 바뀐 영역 정보: https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api , https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info
  - `GetFrameDirtyRects` 는 바뀐 영역을 서로 겹치지 않는 사각형 목록으로 준다.
  - `GetFrameMoveRects` 는 "원본 점 + 대상 사각형" 목록을 준다. 늘이거나 줄이지 않은 단순 이동만 담는다.
  - 화면을 다시 만들려면 move 를 먼저, dirty 를 나중에 적용한다. 앱이 처리하지 못한 갱신이 쌓이면 OS 가 영역을 합쳐서(coalesce) 주므로 실제로 안 바뀐 픽셀이 dirty 영역에 섞일 수 있다(`RectsCoalesced`).
- `AcquireNextFrame`: 한 번 시작한 대기는 취소할 수 없으므로 `INFINITE` 대신 유한 timeout 을 쓴다. 새 프레임은 화면이 갱신되거나 하드웨어 포인터의 모양이나 위치가 바뀔 때 나온다. 이전 프레임을 `ReleaseFrame` 하지 않고 다시 부르면 `DXGI_ERROR_INVALID_CALL`. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe
  - 포인터만 바뀐 프레임에서는 `AccumulatedFrames`, `TotalMetadataBufferSize`, `LastPresentTime` 이 0. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info
- 커서(pointer shape): 포인터가 이미지에 이미 그려져 있을 수도 있고, 그래픽 어댑터가 화면 위에 따로 얹는(overlay) 경우도 있다. `PointerPosition.Visible` 이 참이면 따로 그려진 것이므로 앱이 직접 합성해야 한다. 모양은 `GetFramePointerShape` 으로 받고, 모양이 바뀐 프레임에서만 `PointerShapeBufferSize` 가 0 이 아니다. 좌표는 hot spot 이 아니라 커서 이미지의 좌상단. https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api , https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info
- 포인터 모양 타입 3종: https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_outdupl_pointer_shape_type
  - `MONOCHROME`: 1bpp AND mask 뒤에 같은 크기의 1bpp XOR mask.
  - `COLOR`: 32bpp ARGB.
  - `MASKED_COLOR`: 32bpp. alpha 가 0 이면 화면 픽셀을 그 RGB 로 바꾸고, 0xFF 이면 화면 픽셀과 XOR.
- `DXGI_ERROR_ACCESS_LOST`: 화면에 다른 종류의 이미지가 뜨면 발생한다. 문서 예시는 desktop switch, mode change, DWM 켜짐/꺼짐, 다른 전체화면 앱으로 전환. 인터페이스를 해제하고 새로 만들어야 한다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe
- `DuplicateOutput` 실패 경우: https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput
  - `E_ACCESSDENIED`: 현재 desktop 이미지에 접근 권한이 없다. 원문 "only an application that runs at LOCAL_SYSTEM can access the secure desktop".
  - `DXGI_ERROR_UNSUPPORTED`: 8bpp, non-DWM 모드 등. `EVENT_SYSTEM_DESKTOPSWITCH` 나 `WM_DISPLAYCHANGE` 뒤 다시 시도.
  - `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`: 한 세션에서 동시에 duplication 을 쓰는 프로세스 수가 기본 4개로 제한.
  - `DXGI_ERROR_SESSION_DISCONNECTED`: 세션 연결이 끊김.
  - 한 프로세스는 output 하나에 duplication 을 하나만 가질 수 있지만, output 마다 하나씩은 동시에 가질 수 있다.
- GPU adapter 제약:
  - D3D device 는 그 output 이 연결된 adapter 에서 만들어야 한다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput
  - Microsoft Hybrid 시스템(내장 + 외장 GPU 노트북)에서 외장 GPU 로 DDA 를 돌리면 설계상 `DXGI_ERROR_UNSUPPORTED` 로 실패하고, 우회책은 내장 GPU 에서 실행하는 것이다. 이 KB 3019314 의 적용 대상은 Windows 8.1 로만 적혀 있다. https://learn.microsoft.com/en-us/troubleshoot/windows-client/shell-experience/error-when-dda-capable-app-is-against-gpu
  - Sunshine 은 이 문제를 피하려고 win32u.dll 의 `NtGdiDdDDIGetCachedHybridQueryValue` 를 hook 한다. 소스 주석에 따르면 GPU preference 판정 때문에 DXGI 가 output 을 렌더링 GPU 쪽으로 옮기는(output reparenting) 동작이 DDA 를 깨뜨리기 때문이다. Microsoft 문서에 없는 비공식 우회다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/display_base.cpp#L434-L476
- HDR / 원본 형식:
  - `IDXGIOutput5::DuplicateOutput1` 은 Windows 10 / Server 2016 부터. 받을 형식 목록을 넘기면 전체화면 앱의 원래 back buffer 형식을 변환 없이 받을 수 있고 R10G10B10A2 같은 high color 도 받는다. 목록에 없는 형식이면 DXGI 가 목록 중 하나로 변환한다. 목록에 `B8G8R8A8_UNORM` 을 항상 넣으라고 권한다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_5/nf-dxgi1_5-idxgioutput5-duplicateoutput1
  - Microsoft 공식 샘플은 `IDXGIOutput6` 가 있으면 `R16G16B16A16_FLOAT`(FP16)을 목록에 넣는다. https://github.com/microsoft/Windows-classic-samples/blob/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/DXGIDesktopDuplication/cpp/DuplicationManager.cpp#L59-L89
  - 같은 샘플은 "DuplicateOutput1 에는 per-monitor DPI awareness 가 필요하다" 는 주석과 함께 `SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2)` 를 부른다. API reference 에는 이 조건이 없고 샘플 주석이 유일한 근거다. https://github.com/microsoft/Windows-classic-samples/blob/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/DXGIDesktopDuplication/cpp/DesktopDuplication.cpp#L487-L488
  - 모니터가 HDR 인지는 `IDXGIOutput6::GetDesc1` 의 `ColorSpace` 로 안다. `RGB_FULL_G2084_NONE_P2020` 이면 advanced color, `RGB_FULL_G22_NONE_P709` 이면 SDR. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_6/ns-dxgi1_6-dxgi_output_desc1
- 보호 콘텐츠:
  - `ProtectedContentMaskedOut` 이 TRUE 이면 보호 콘텐츠가 이미 검게 가려졌을 수 있다. 문서는 원격 사용자에게 알리는 용도로 쓰라고 한다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info
  - Sunshine 주석에 따르면 보호 콘텐츠가 계속 떠 있어도 이 값이 TRUE/FALSE 를 오가서 10초 간격으로만 경고한다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/display_base.cpp#L155-L161
  - 앱은 `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` 로 자기 창을 캡처 결과에서 뺄 수 있다. Windows 10 2004 부터, 그 전 버전에서는 `WDA_MONITOR`(내용 없이 빈 창)처럼 동작. DWM 합성 중에만 효과. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity
- 전체화면 exclusive 앱: 2013년 Microsoft 블로그(archive)에 따르면 DirectX 11.1 이전의 exclusive 모드 앱은 대체로 DDA 로 잡히지 않고, 11.1 이후 앱은 스스로 opt-out 하지 않는 한 잡힌다. https://learn.microsoft.com/en-us/archive/blogs/dsui_team/ways-to-capture-the-screen
- 회전: 받는 surface 는 항상 회전 전 방향이고 회전된 화면이 그 안에 담겨 온다(예: 768x1024 세로 모드에서도 1024x768 surface). 앱이 직접 돌려야 한다. https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api
- secure desktop / 잠금 화면: LOCAL_SYSTEM 권한이 필요하다(위 `E_ACCESSDENIED`). Winlogon 은 window station 과 desktop 에 local system 전체 접근을 준다. https://learn.microsoft.com/en-us/windows/win32/secauthn/responsibilities-of-winlogon . Sunshine 은 `DuplicateOutput` 전에 매번 `OpenInputDesktop` + `SetThreadDesktop` 으로 캡처 스레드를 현재 input desktop 에 붙인다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/misc.cpp#L224-L241 , https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/display_base.cpp#L78-L90

### 1.2 Windows.Graphics.Capture (WGC)

- 최소 버전: `GraphicsCaptureSession` 은 Windows 10 1803 (10.0.17134). https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession . Win32 앱이 picker 없이 모니터를 지정하는 `IGraphicsCaptureItemInterop::CreateForMonitor(HMONITOR)` 는 Windows 10 1903 (18362). https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createformonitor
- 기본 동작: 사용자가 시스템 picker UI 로 대상을 고르고, 캡처 중인 대상 둘레에 노란 테두리가 그려진다. https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture . WinRT 쪽에서 `DisplayId`/`WindowId` 로 picker 없이 item 을 만들려면 package manifest 에 `graphicsCaptureProgrammatic` capability 가 필요하다. https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/app-capability-declarations
- 커서 포함 여부: `IsCursorCaptureEnabled`, Windows 10 2004 (10.0.19041) 부터. 켜고 끄기만 되고 모양을 따로 받지는 못한다. https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.iscursorcaptureenabled
- 테두리 끄기(`IsBorderRequired`):
  - 문서 표기 "Windows 10, version 2104 (introduced in 10.0.20348.0)", UniversalApiContract v12. 문서 moniker 는 winrt-20348, 22000(Windows 11 21H2) 이후. https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired
  - 끄려면 먼저 `GraphicsCaptureAccess.RequestAccessAsync(GraphicsCaptureAccessKind.Borderless)` 로 사용자 동의 prompt 를 거쳐야 한다. 사용자가 거부하면 false 설정은 성공하지만 무시되고 테두리가 나온다. 이 호출에는 package manifest 의 `graphicsCaptureWithoutBorder` capability 선언이 필요하다. 다른 앱이 같은 대상에 true 로 두면 테두리가 나온다. https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired , https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscaptureaccess.requestaccessasync
  - 20348 은 Windows Server 2022 의 build 번호이고, Windows 10 client 마지막 버전 22H2 는 build 19045 다. https://learn.microsoft.com/en-us/windows-hardware/drivers/display/iddcx-versions . 따라서 Windows 10 client 에서는 테두리를 끌 수 없다고 봐야 한다(두 문서를 맞대어 본 추론). Windows 10 19045 에서 `SetIsBorderRequired` 호출이 `E_NOINTERFACE` 로 실패한 보고는 미확인(2차 출처). https://github.com/openai/codex/issues/25178
  - unpackaged Win32 앱에서 동의가 어떻게 동작하는지는 문서에 없다. Win32 WGC 샘플은 `RequestAccessAsync(Borderless)` 를 부른 뒤 결과와 상관없이 값을 설정하고 "user or system policy 가 거부해도 설정은 안전하다" 고 주석을 단다. https://github.com/robmikh/Win32CaptureSample/blob/49fefe79fd9b11025f0b5eb91783a98888516070/Win32CaptureSample/App.cpp#L321-L331 . Sunshine 은 `RequestAccessAsync` 없이 `ApiInformation::IsPropertyPresent` 로 속성 존재만 확인하고 false 로 설정한다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/display_wgc.cpp#L149-L157 . unpackaged 앱에서 prompt 가 뜨는지, 자동 허용되는지는 미확인.
- dirty region, 갱신 간격: `GraphicsCaptureSession.DirtyRegionMode`, `MinUpdateInterval`, `Direct3D11CaptureFrame.DirtyRegions` 는 문서 moniker 가 winrt-26100(Windows 11 24H2 SDK)부터이고 enum ContractVersion 은 UniversalApiContract v19. 설명 문장은 문서에 없다. https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.dirtyregionmode , https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturedirtyregionmode , https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.direct3d11captureframe
  - robmikh 샘플은 `IsPropertyPresent("DirtyRegionMode")` 로 기능 존재를 확인한 뒤 쓴다. https://github.com/robmikh/Win32CaptureSample/blob/49fefe79fd9b11025f0b5eb91783a98888516070/Win32CaptureSample/App.cpp#L57
  - Sunshine 은 `MinUpdateInterval(4ms)`(250Hz)를 설정하고 실패하면 "60fps 로 제한될 수 있다" 는 로그를 남긴다. 기본 상한이 60fps 라는 Microsoft 문서 근거는 없다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/display_wgc.cpp#L158-L166
- HDR: Windows HD color 가 켜진 시스템에서는 형식이 `B8G8R8A8_UNORM` 이 아닐 수 있고, 문서는 파이프라인 전체에 `R16G16B16A16_FLOAT` 를 쓰라고 권한다. https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture
- SYSTEM 서비스 모드와 맞지 않음(실사용 근거): Sunshine 문서는 wgc 캡처 방식이 "not compatible with the Sunshine service" 라고 적는다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/docs/configuration.md#L2231-L2235 . Sunshine 서비스 모드는 SYSTEM token 을 복제해 `TokenSessionId` 를 console session 으로 바꾸고 `CreateProcessAsUser` 로 `winsta0\default` 에 띄우는 구조다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/tools/sunshinesvc.cpp#L106-L156 . 즉 SYSTEM 권한으로 사용자 세션에서 돌아도 WGC 가 동작하지 않았다. 서비스 모드에서 `0x80070424` 로 실패한 보고: https://github.com/LizardByte/Sunshine/issues/2846 . Microsoft 문서에는 명시가 없어 이유는 미확인.
- secure desktop: WGC 로 캡처하면 UAC prompt 가 보이지 않고 스트림이 멈춘다는 Sunshine 보고가 있다(DDA 에서는 보였음). Microsoft 문서에는 명시 없음. https://github.com/LizardByte/Sunshine/issues/3487
- hybrid GPU 환경의 동작, 보호 콘텐츠 처리는 Microsoft 문서에서 찾지 못했다.

### 1.3 GDI `BitBlt`

- 최소 Windows 2000. `CAPTUREBLT` 는 위에 겹친 layered window 까지 결과에 포함한다. source 와 destination 이 다른 device 이거나 source 에 rotation/shear transform 이 걸려 있으면 에러. https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-bitblt
- 느릴 수 있고, 대상 bitmap 의 해상도와 비트 깊이를 화면과 같게 맞추면 색 변환이 빠져 빨라진다. 멀티미디어 출력 같은 일부 내용은 잡히지 않는다(Microsoft 블로그 archive, 2013). https://learn.microsoft.com/en-us/archive/blogs/dsui_team/ways-to-capture-the-screen
- dirty rect 가 없어 바뀐 곳을 직접 찾아야 한다. TigerVNC Windows 서버는 BitBlt(선택적으로 `CAPTUREBLT`)에 polling 이나 window message hook 을 더해 변경을 감지한다. https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/win/rfb_win32/DeviceFrameBuffer.cxx#L94-L111
- 커서: RustDesk 주석 "CAPTUREBLT enable layered window but also make cursor blinking". https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/dxgi/gdi.rs#L92-L102 . BitBlt 결과에 커서가 기본으로 빠진다는 Microsoft 공식 문구는 찾지 못했다, 미확인(2차 출처).

### 1.4 요약 비교

| 항목 | DDA | WGC | GDI BitBlt |
|---|---|---|---|
| 최소 버전 | Win8 (`DuplicateOutput1` 은 Win10) | 1803, Win32 모니터 지정은 1903 | Win2000 |
| dirty/move | dirty + move 둘 다 | dirty 만, 문서 moniker 26100 이후 | 없음 |
| 커서 | 위치와 모양을 따로 받음 | 포함/제외 1 bit (2004+) | 직접 그려야 함 |
| HDR | `DuplicateOutput1` + FP16 | FP16 frame pool | 없음 |
| 테두리 | 없음 | 있음, 끄기는 20348+ 와 사용자 동의 | 없음 |
| secure desktop / SYSTEM | LOCAL_SYSTEM 이면 가능 | SYSTEM 서비스 모드에서 불가 (Sunshine 기준) | 해당 desktop 권한 필요 (추론) |
| hybrid GPU | 외장 GPU 에서 `UNSUPPORTED` | 문서 없음 | 해당 없음 |
| 동시 사용 제한 | 세션당 4 프로세스 | 문서 없음 | 없음 |

출처는 위 각 항목과 같다.

**RustDesk 구현 (캡처):**

- 기본은 DDA 이고, device 생성이나 `DuplicateOutput` 이 실패하면 GDI 로 넘어간다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/dxgi/mod.rs#L75-L192
- `IDXGIOutput6` 가 있으면 `DuplicateOutput1` 로 FP16 을 받아 tone-map 한다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/dxgi/mod.rs#L194-L230 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/dxgi/hdr.rs
- dirty/move rect 는 쓰지 않고 주석으로만 언급한다(같은 파일 L139).
- 프레임이 계속 오지 않으면(WouldBlock) GDI 로 바꾸고, input desktop 이 바뀌면 캡처 서비스를 다시 시작한다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/video_service.rs#L779-L786 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/video_service.rs#L880-L895
- Windows 에서는 커서를 영상에 넣지 않는다(`is_cursor_embedded()` 가 false). `GetCursorInfo`/`GetIconInfo` 로 모양을 따로 보내고 viewer 가 그린다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/common/mod.rs#L281-L285 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L207-L262
- WGC 는 쓰지 않는다(repo 에 관련 코드 없음). Magnification API 캡처(`mag.rs`)는 privacy mode 용이다.

## 2. 권한 모델: UAC secure desktop, 잠금 화면, 로그인 화면

소스 링크는 고정 커밋 기준이다(RustDesk `58ff78a8`, TigerVNC `3dcfda19`).

### 2.1 Session 0 격리

- Windows Vista 부터 서비스는 사용자와 직접 상호작용할 수 없다. 모든 서비스는 session 0 에서 돌고, Fast User Switching 도 Terminal Services 위에서 구현되어 있다. 그래서 서비스가 사용자 세션 화면에 직접 접근할 방법이 없다. https://learn.microsoft.com/en-us/windows/win32/services/interactive-services
- Microsoft 권장 방식: 별도 GUI 프로세스를 `CreateProcessAsUser` 로 사용자 세션에 띄우고, 서비스와는 named pipe 같은 IPC 로 통신한다. IPC 에 ACL 을 걸지 않으면 서비스 인터페이스가 네트워크에 노출될 수 있다고 문서가 경고한다. https://learn.microsoft.com/en-us/windows/win32/services/interactive-services
- `NoInteractiveServices` 기본값이 1 이라 `SERVICE_INTERACTIVE_PROCESS` 는 쓸 수 없다. Windows 7 까지는 기본값이 0 이었다. https://learn.microsoft.com/en-us/windows/win32/services/interactive-services

### 2.2 세션 찾기와 변화 감지

- `WTSGetActiveConsoleSessionId` 는 물리 콘솔에 붙은 세션 ID 만 돌려준다. 콘솔이 붙거나 떨어지는 중이면 `0xFFFFFFFF`. 최소 Vista / Server 2008. https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-wtsgetactiveconsolesessionid
- RDP 세션까지 다루려면 `WTSEnumerateSessions` 로 `WTS_SESSION_INFO`(SessionId, pWinStationName, State) 목록을 받아 직접 고른다. 최소 Vista. https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsenumeratesessionsw
- 세션 변화 알림 경로는 두 가지다. 창은 `WTSRegisterSessionNotification` 을 등록해 `WM_WTSSESSION_CHANGE` 를 받고, 서비스는 `HandlerEx` 에서 `SERVICE_CONTROL_SESSIONCHANGE` 를 받는다(lpEventData 는 `WTSSESSION_NOTIFICATION`). 서비스는 로그온 시도 전에 완전히 올라와 있어야 로그온 알림을 받는다. https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsregistersessionnotification , https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nc-winsvc-lphandler_function_ex
- 이벤트 종류: `WTS_CONSOLE_CONNECT`(0x1), `WTS_CONSOLE_DISCONNECT`(0x2), `WTS_REMOTE_CONNECT`/`DISCONNECT`(0x3/0x4), `WTS_SESSION_LOGON`/`LOGOFF`(0x5/0x6), `WTS_SESSION_LOCK`/`UNLOCK`(0x7/0x8), `WTS_SESSION_REMOTE_CONTROL`(0x9), `WTS_SESSION_DESKTOP_READY`(0xF). https://learn.microsoft.com/en-us/windows/win32/termserv/wm-wtssession-change
- 잠김 여부는 `WTSQuerySessionInformation(WTSSessionInfoEx)` 의 `WTSINFOEX_LEVEL1.SessionFlags` 로 알 수 있다. Windows 7 / Server 2008 R2 에서는 코드 결함으로 LOCK 과 UNLOCK 값이 뒤바뀌어 나온다. https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/ns-wtsapi32-wtsinfoex_level1_w

### 2.3 사용자 세션에 helper 를 띄우는 방법

- 방법 A, 사용자 토큰: `WTSQueryUserToken` 은 호출자가 LocalSystem 이고 `SE_TCB_NAME` 권한이 있어야 한다. 최소 Vista. https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsqueryusertoken
  - 로그온한 사용자가 없는 세션에서는 `ERROR_NO_TOKEN` 으로 실패한다고 알려져 있으나, 현재 Learn 페이지에 오류 코드 설명이 없어 미확인(2차 출처). https://www.ibm.com/support/pages/apar/JR49667
  - 이 토큰으로 띄운 helper 는 사용자 권한(medium IL, IL = integrity level)으로 돌기 때문에 UAC 창과 로그인 화면에 닿지 못한다(근거는 2.5, 2.6).
- 방법 B, SYSTEM 토큰의 세션 ID 바꾸기: `CreateProcessAsUser` 는 토큰에 기록된 세션에서 프로세스를 실행한다. 세션을 바꾸려면 `SetTokenInformation(TokenSessionId)` 를 쓰고, 이때 "Act As Part Of the Operating System"(`SE_TCB_NAME`) 권한이 필요하다. https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessasuserw , https://learn.microsoft.com/en-us/windows/win32/api/winnt/ne-winnt-token_information_class
- 방법 C, 실제 구현들이 쓰는 방식: 대상 세션의 `winlogon.exe` 를 찾아(`CreateToolhelp32Snapshot` + `ProcessIdToSessionId`) 그 토큰을 `OpenProcessToken` 으로 연다. 이 토큰은 이미 SYSTEM 이면서 그 세션에 속해 있으므로 `SetTokenInformation` 없이 `CreateProcessAsUser` 를 부를 수 있다. 사용자가 로그온하지 않은 로그인 화면에서도 동작한다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.cc#L126 , https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/win/winvnc/VNCServerService.cxx#L68
- `CreateProcessAsUser` 주의점: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessasuserw
  - `SE_INCREASE_QUOTA_NAME` 권한이 필요하고, 토큰이 assignable 이 아니면 `SE_ASSIGNPRIMARYTOKEN_NAME` 도 필요하다.
  - `lpDesktop` 이 NULL 이면 부모의 window station 과 desktop 을 물려받는다. 화면에 보이게 하려면 `"winsta0\default"` 를 지정한다.
  - 사용자 프로필과 환경 블록은 자동으로 준비되지 않으므로 `LoadUserProfile`, `CreateEnvironmentBlock` 을 직접 부른다.
  - 세션을 넘는 handle 상속은 되지 않는다.

### 2.4 데스크톱 전환 따라가기

- winsta0 에는 기본으로 Default, ScreenSaver, Winlogon 세 desktop 이 있고, 입력을 받는 desktop 은 한 번에 하나다. Ctrl+Alt+Del 을 누르거나 UAC 대화상자가 열리면 Winlogon desktop 으로 바뀐다. Winlogon desktop 의 security descriptor 는 LocalSystem 을 포함한 극소수 계정만 허용하므로 일반 앱은 접근도 전환도 못 한다. https://learn.microsoft.com/en-us/windows/win32/winstation/desktops
- 윈도 메시지와 hook 은 같은 desktop 안에서만 동작한다. https://learn.microsoft.com/en-us/windows/win32/winstation/desktops
- `OpenInputDesktop` 은 현재 입력 desktop 의 핸들을 준다. 호출 프로세스에 입력을 받을 수 있는 window station 이 있어야 한다. 연결이 끊긴 세션에서는 다시 연결될 때 활성화될 desktop 을 돌려준다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openinputdesktop
- `SetThreadDesktop` 은 호출 스레드가 현재 desktop 에 창이나 hook 을 가지고 있으면 실패한다. 그래서 캡처와 입력은 창이 없는 전용 스레드에서 해야 한다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddesktop
- DXGI 캡처는 desktop 이 바뀌면 `AcquireNextFrame` 이 `DXGI_ERROR_ACCESS_LOST` 를 돌려주므로 duplication 인터페이스를 다시 만들어야 한다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe
- `DuplicateOutput` 은 권한이 없으면 `E_ACCESSDENIED` 를 돌려준다. 문서 원문: "only an application that runs at LOCAL_SYSTEM can access the secure desktop". 세션이 끊겨 있으면 `DXGI_ERROR_SESSION_DISCONNECTED`, 지원하지 않는 모드면 `DXGI_ERROR_UNSUPPORTED` 이며 이 경우 `EVENT_SYSTEM_DESKTOPSWITCH` 나 `WM_DISPLAYCHANGE` 알림 뒤 다시 시도하라고 한다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput
- 판별 방법: 스레드 desktop 과 입력 desktop 의 이름을 `GetUserObjectInformation(UOI_NAME)` 으로 비교하고, 다르면 `OpenInputDesktop` 후 `SetThreadDesktop`. RustDesk 와 TigerVNC 가 같은 방식을 쓴다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.cc#L296 , https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/win/rfb_win32/Service.cxx#L173
- RustDesk 는 마우스나 키 입력을 주입하기 직전마다, 그리고 서비스 루프에서 에러가 난 뒤마다 `try_change_desktop()` 을 부른다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/input_service.rs#L1118 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/service.rs#L278

### 2.5 UAC secure desktop

- `PromptOnSecureDesktop` 기본값은 1 이라 모든 권한 상승 요청이 secure desktop 으로 간다. https://learn.microsoft.com/en-us/windows/security/application-security/application-control/user-account-control/settings-and-configuration
- 문서 원문: "Only Windows processes can access the secure desktop". https://learn.microsoft.com/en-us/windows/security/application-security/application-control/user-account-control/how-it-works
- Windows Server 2019 부터, 그리고 지원 중인 클라이언트 OS 에서는 secure desktop 에 클립보드를 붙여 넣을 수 없다. https://learn.microsoft.com/en-us/windows/security/application-security/application-control/user-account-control/how-it-works
- `EnableUIADesktopToggle`(기본값 0)을 켜면 UIAccess 프로그램(Remote Assistance 포함)이 standard user 의 권한 상승 창을 secure desktop 없이 사용자 desktop 에 띄울 수 있다. 관리자용 동의 창 동작은 바뀌지 않는다. https://learn.microsoft.com/en-us/windows/security/application-security/application-control/user-account-control/settings-and-configuration
- uiAccess 로도 system IL UI 에는 닿지 못한다. 원문: "None of the previously listed scenarios provide access to UI running under the system IL. This is only possible if the process is launched in the user account control (UAC) desktop under SYSTEM". https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview
- 서비스 없이 동작하는 Quick Assist 도 "In some scenarios, the helper does require the sharer to respond to application permission prompts (User Account Control)" 라고 적는다. 즉 서비스가 없으면 UAC 창은 host PC 앞의 사람이 직접 눌러야 한다. https://learn.microsoft.com/en-us/windows/client-management/client-tools/quick-assist

### 2.6 UIPI 와 uiAccess

- `SendInput` 은 UIPI(User Interface Privilege Isolation) 적용을 받는다. IL 이 같거나 낮은 프로세스에만 입력을 넣을 수 있고, 막혀도 반환값이나 `GetLastError` 로는 UIPI 때문인지 알 수 없다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
- uiAccess=true 의 조건: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview , https://learn.microsoft.com/en-us/windows/security/application-security/application-control/user-account-control/settings-and-configuration
  - Authenticode 서명. 서명 검사는 설정과 관계없이 항상 한다.
  - 보호된 위치에 설치: `%ProgramFiles%`, `%ProgramFiles(x86)%`, `%SystemRoot%\system32`. `EnableSecureUIAPaths` 기본값이 1 이라 이 조건이 적용된다.
  - manifest 에 `uiAccess="true"`.
- 관리자가 아닌 사용자가 uiAccess 앱을 실행하면 "medium+" IL 이 되어 "관리자 권한으로 실행" 한 창을 조작하지 못한다. 관리자가 실행하면 high IL. Microsoft 는 assistive technology 가 아닌 앱에 uiAccess 를 쓰지 말라고 명시한다. https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview

### 2.7 Ctrl+Alt+Del (SAS, Secure Attention Sequence)

- `SendSAS`(sas.dll) 최소 Windows 7 / Server 2008 R2. 호출하려면 서비스로 실행되거나 manifest 에 uiAccess=true 가 있어야 한다. 서비스가 아니면 현재 사용자나 LocalSystem 으로 실행되어야 하고 UAC 가 켜져 있어야 한다. 서비스가 다른 프로세스의 토큰을 impersonate 한 상태에서 부르면 그 토큰의 세션으로 SAS 가 간다. https://learn.microsoft.com/en-us/windows/win32/api/sas/nf-sas-sendsas
- 정책 설정도 필요하다. 위치는 Computer Configuration > Administrative Templates > Windows Components > Windows Logon Options > "Disable or enable software Secure Attention Sequence", 값 이름 `SoftwareSASGeneration`, 키 `HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\System`. 선택지는 None, Services, Ease of Access applications, Services and Ease of Access applications. 설정하지 않으면 "only Ease of Access applications running on the secure desktop can simulate the SAS" 이므로 기본 상태에서는 서비스의 `SendSAS` 가 막힌다. Policy CSP 로는 Win10 2004 + KB5005101 또는 Win11 21H2 이상에서 설정 가능. https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-winlogon
- 레지스트리 숫자 값:
  - 3 = Services and Ease of Access 는 Microsoft 보관 블로그에서 확인. https://learn.microsoft.com/en-us/archive/blogs/technet/itasupport/sendsas-step-by-step
  - 0 = None, 1 = Services, 2 = Ease of Access 는 RustDesk 코드 주석 기준이며 Microsoft 1차 문서에서는 찾지 못했다. 미확인(2차 출처). https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L949
- `SendInput` 으로 Ctrl+Alt+Del 을 흉내 내도 SAS 가 되지 않는다는 점은 미확인(2차 출처). https://en.wikipedia.org/wiki/Secure_attention_key
- RustDesk 흐름: helper 가 IPC `Data::SAS` 로 서비스에 요청한다. 서비스의 `send_sas()` 는 정책 값이 1 이나 3 이 아니면 잠시 1 로 바꾸고 `SendSAS(FALSE)` 를 부른 뒤 원래 값으로 되돌린다. 설치 스크립트(`get_after_install`)에서도 `SoftwareSASGeneration=1` 을 넣어 둔다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L949 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L1523
- TigerVNC 흐름: helper 가 `Global\SessionEventTigerVNCCad` 이벤트를 set 하면 session 0 의 서비스 루프가 `_SendSAS(FALSE)` 를 부른다. https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/win/rfb_win32/Service.cxx#L257 , https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/win/winvnc/VNCServerService.cxx#L135

### 2.8 잠금 화면과 로그인 화면

- 사용자가 로그온하는 동안에는 Winlogon desktop 이 활성 상태이고, 이 desktop 은 LocalSystem 등 일부 계정만 접근할 수 있다. 그래서 대상 세션 안의 SYSTEM helper 가 필요하다. https://learn.microsoft.com/en-us/windows/win32/winstation/desktops
- Winlogon 은 local system 에 전체 접근을 주고, 로그온한 사용자에게는 window station 읽기와 application desktop 전체 접근을 준다. https://learn.microsoft.com/en-us/windows/win32/secauthn/responsibilities-of-winlogon
- RustDesk 는 세션 사용자 이름이 비었거나 SYSTEM 이면 로그인 전 상태(`is_prelogin`), 세션에 `LogonUI.exe` 가 있으면 로그온 UI 상태(`is_logon_ui`)로 본다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L1261
- Windows 10 이후 잠금 화면(LockApp)이 Winlogon desktop 이 아니라 Default desktop 에서 돈다는 보고가 있으나 미확인(2차 출처). https://github.com/trycua/cua/issues/1745

### 2.9 RDP 세션과 Fast User Switching

- 연결이 끊긴 RDP 세션은 `DuplicateOutput` 이 `DXGI_ERROR_SESSION_DISCONNECTED` 를 돌려주므로 캡처할 수 없다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput
- RustDesk 의 `get_current_session(include_rdp)` 는 console WinStation 을 먼저 고른다. 없으면 활성 세션 가운데 "rdp" 나 "ica" 접두사가 붙었거나 `WTSClientProtocolType` 이 RDP 인 세션(Hyper-V Enhanced Session 포함)을 고른다. 서비스 루프는 timeout 마다 세션을 다시 확인하고, 바뀌었거나 helper 가 죽었으면 helper 를 다시 띄운다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.cc#L565 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L665
- TigerVNC 는 1초마다 console session ID 를 비교해 바뀌면 helper 를 다시 띄운다. https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/win/winvnc/VNCServerService.cxx#L135

**RustDesk 구현:** LocalSystem 서비스(`run_service`)가 활성 세션의 `winlogon.exe` 토큰으로 `--server` helper 를 SYSTEM 으로 띄운다(`launch_privileged_process` 에서 `LaunchProcessWin`). helper 는 입력 주입 직전과 에러 때마다 `try_change_desktop` 으로 입력 desktop 을 따라간다. SAS 는 helper 가 IPC 로 서비스에 부탁한다. 설치 없이 실행하는 portable 모드는 UAC 승인 후 `run_as_system` 으로 SYSTEM 프로세스를 만들고, 캡처 프레임과 입력을 shared memory 로 주고받는다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L832 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.cc#L233 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L2410 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/portable_service.rs

## 3. 입력 주입 (host 쪽)

RustDesk, rdev fork, TigerVNC 링크는 commit SHA 로 고정한 permalink 다.

### 3.1 SendInput 공통 사항

- `SendInput` 은 Windows 2000 부터. 한 번 호출로 넣은 이벤트 배열은 다른 입력과 섞이지 않고 순서대로 들어간다. 반환값은 실제로 넣은 이벤트 수. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
- UIPI 적용: 호출 프로세스와 IL 이 같거나 낮은 앱에만 입력이 들어간다. 원문 "neither GetLastError nor the return value will indicate the failure was caused by UIPI blocking". 막혀도 알 방법이 없다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
- `SendInput` 은 현재 키보드 상태를 초기화하지 않는다. 이미 눌린 키가 있으면 주입한 이벤트와 충돌할 수 있으므로 `GetAsyncKeyState` 로 확인해 바로잡으라고 문서가 권한다. 원격 제어에서는 viewer 가 포커스를 잃었을 때 host 에 눌린 채 남는 키(stuck key)를 풀어 주는 로직이 필요하다는 뜻이다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
- UIAccess 프로세스는 `SendInput` 으로 어떤 창이든 조작할 수 있고, low-level hook, raw input, `GetKeyState`, `GetAsyncKeyState` 로 모든 IL 의 입력을 읽을 수 있다. 조건은 신뢰할 수 있는 인증서 서명 + 관리자만 쓸 수 있는 위치(`%ProgramFiles%`, `%WinDir%`) 설치. system IL(secure desktop) 에는 닿지 못한다는 점은 2.5 참고. https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/security-policy-settings/user-account-control-only-elevate-uiaccess-applications-that-are-installed-in-secure-locations
- 주입한 입력은 앱이 구분할 수 있다. low-level hook 구조체에 키보드는 `LLKHF_INJECTED`(0x10), `LLKHF_LOWER_IL_INJECTED`(0x02), 마우스는 `LLMHF_INJECTED`(0x01), `LLMHF_LOWER_IL_INJECTED`(0x02) flag 가 붙는다. 그래서 게임이나 anti-cheat 는 주입 입력을 가려낼 수 있다. 실제로 어떤 앱이 무시하는지는 앱마다 달라 미확인. https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-kbdllhookstruct , https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-msllhookstruct
- `dwExtraInfo` 에는 호출하는 쪽이 원하는 값을 넣을 수 있다. RustDesk 는 자기가 주입한 입력을 표시하려고 `ENIGO_INPUT_EXTRA_VALUE = 100` 을 넣는다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/enigo/src/win/win_impl.rs#L20-L80
- 입력은 injector thread 가 붙어 있는 desktop 으로만 간다. RustDesk 는 주입 전마다 `try_change_desktop()` 으로 input desktop 을 따라간다(2.4 참고). 이 코드는 TigerVNC 구현에서 가져왔다고 주석에 적혀 있다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L1040-L1056 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.cc#L292-L345

### 3.2 마우스

- `MOUSEEVENTF_ABSOLUTE` 를 쓰면 dx, dy 는 0..65535 로 정규화된 좌표다. (0,0) 은 왼쪽 위, (65535,65535) 는 오른쪽 아래. 이 flag 만 쓰면 primary monitor 에 맞춰지고, `MOUSEEVENTF_VIRTUALDESK` 를 함께 써야 virtual desktop 전체에 맞춰진다. 최소 Windows 2000. https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput
- 상대 이동(ABSOLUTE 없음)은 사용자의 포인터 속도 설정과 가속 threshold 영향을 받아 요청한 거리가 최대 4배까지 늘어날 수 있다. 원격 제어는 절대 좌표를 써야 위치가 맞는다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput
- virtual screen 은 모든 모니터를 감싸는 사각형이고 `SM_XVIRTUALSCREEN`, `SM_YVIRTUALSCREEN` 이 그 왼쪽 위 좌표다(음수 가능). `GetSystemMetrics` 문서에는 "not DPI aware, and should not be used if the calling thread is per-monitor DPI aware" 라고 적혀 있다(5절과 연결). https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getsystemmetrics
- 픽셀을 0..65535 로 바꿀 때의 반올림 규칙은 문서에 없고 양 끝점만 정의되어 있다. 오픈소스 구현 세 곳이 서로 다른 식을 쓴다. 해상도별로 `GetCursorPos` 로 되읽어 비교하는 테스트가 필요하다.
  - RustDesk enigo: `(x - SM_XVIRTUALSCREEN) * 65535 / SM_CXVIRTUALSCREEN`. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/enigo/src/win/win_impl.rs#L128-L137
  - rdev fork: `(x + 1) * 65535 / SM_CXVIRTUALSCREEN`, origin 을 빼지 않는다. https://github.com/rustdesk-org/rdev/blob/a361d86a8b0245f3618a9efb375c149530a6b599/src/windows/simulate.rs#L178-L190
  - TigerVNC: `(x - SM_XVIRTUALSCREEN) * 65535 / (SM_CXVIRTUALSCREEN - 1)`, primary monitor 안쪽 좌표에는 구형 `mouse_event` 를 쓴다. https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/win/rfb_win32/SInput.cxx#L110-L135
- `MOUSEEVENTF_MOVE_NOCOALESCE` 를 쓰면 `WM_MOUSEMOVE` 를 합치지 않는다(XP, 2000 미지원). https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput
- 휠:
  - 한 칸은 `WHEEL_DELTA` = 120. `MOUSEEVENTF_HWHEEL`(가로 휠)은 Vista 부터. `mouseData` 를 같이 쓰므로 WHEEL 과 XDOWN/XUP 을 한 이벤트에 함께 넣을 수 없다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput
  - 120 을 기준으로 잡은 이유는 칸이 없는 고해상도 휠이 더 작은 값을 더 자주 보낼 수 있게 하려는 것이고, 앱은 그 값을 누적하거나 부분 스크롤로 처리한다. 따라서 원격 전송도 칸 수가 아니라 원래 delta 값을 그대로 보내야 부드러운 스크롤이 유지된다. https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
- XBUTTON(뒤로/앞으로)은 `MOUSEEVENTF_XDOWN`/`XUP` 에 `mouseData` 로 `XBUTTON1`(0x0001) 또는 `XBUTTON2`(0x0002). https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput
- RustDesk Windows host 는 trackpad 가 아닌 휠 값에 `WHEEL_DELTA` 를 곱해 칸 단위로 보낸다. Windows trackpad 스크롤은 코드에 "TODO: support track pad on win." 으로 남아 있다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/input_service.rs#L1231-L1285

### 3.3 키보드: scancode, extended key, Unicode

- `KEYEVENTF_SCANCODE` 를 쓰면 `wScan` 이 키를 정하고 `wVk` 는 무시된다. 원문 "The virtual key value of a key can change depending on the current keyboard layout ... but the scan code will always be the same". https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput
- `KEYEVENTF_EXTENDEDKEY` 는 scan code 앞에 0xE0 이 붙는 키라는 뜻이다. 문서가 extended key 로 정의하는 키: 오른쪽 Alt/Ctrl, 숫자 키패드 왼쪽 묶음의 Insert, Delete, Home, End, Page Up, Page Down, 화살표, Break(Ctrl+Pause), Print Screen, 숫자 키패드 `/` 와 Enter, Windows 키, Application 키. 예외로 Num Lock 은 extended key 가 아니고 Pause 와 같은 scan code 에서 extended flag 유무로만 갈린다. 오른쪽 Shift 도 extended key 가 아니며 별도 scan code 0x36 을 쓴다. https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input
- scan code 표 기준 Pause 는 `0xE11D45`, Print Screen 은 `0xE037`. Alt+Print Screen 은 SysRq 코드(0x54), Ctrl+Pause 는 Break 코드가 나온다. https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input
- 실제 구현은 Pause 를 따로 처리한다. TigerVNC 는 주석에 "Windows has some bug where it doesn't look up scan code 0x45 properly" 라고 적고 0x45 를 `VK_NUMLOCK` 이나 `VK_PAUSE` 로 바꿔 보낸다. https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/win/rfb_win32/SInput.cxx#L243-L276 . RustDesk 도 Windows peer 로 보내는 Pause 는 "Windows has no normal scan code for Pause" 라며 legacy control key 로 보낸다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/keyboard.rs#L1247-L1275
- `KEYEVENTF_UNICODE` 는 `VK_PACKET` 키 입력을 만든다. `wVk` 는 0, `wScan` 에 UTF-16 문자 하나, `KEYUP` 과만 같이 쓸 수 있다. `TranslateMessage` 를 거치면 그 문자로 `WM_CHAR` 가 만들어진다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput
  - 보충 평면 문자(예: emoji)는 surrogate pair 로 두 번 나눠 보내야 한다. `SendInput` 문서에도 touch keyboard 가 surrogate 로 입력을 보낸다는 언급이 있다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
  - 앱마다 `VK_PACKET` 처리 품질이 다르다. Windows Terminal 은 1.14 전까지 이 문자를 0x7FFF 로 잘라 엉뚱한 글자가 입력됐다. https://github.com/microsoft/terminal/issues/12977
- `MapVirtualKeyEx` 는 넘겨준 HKL(키보드 layout 핸들) 기준으로 VK 와 scan code 를 바꾼다. `MAPVK_VK_TO_VSC_EX`(Vista 이상)만 extended key 의 0xE0/0xE1 prefix 를 상위 바이트로 돌려주고, `MAPVK_VK_TO_VSC` 는 prefix 를 주지 않는다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mapvirtualkeyexw
- 키보드 layout 은 thread 마다 따로 있다. `GetKeyboardLayout(idThread)` 로 조회하고, 바뀌면 `WM_INPUTLANGCHANGE` 로 알 수 있다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getkeyboardlayout
- RustDesk enigo 는 scan code 가 0 이면 `GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow()))` 로 얻은 layout 으로 `MapVirtualKeyExW(vk, 0, layout)` 를 부르는데, 이 layout 을 static 변수에 처음 한 번만 저장한다. 그 뒤 host 입력 언어가 바뀌어도 반영되지 않는 구조다(코드를 읽고 판단). 반면 rdev fork 는 호출할 때마다 foreground layout 을 다시 읽는다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/enigo/src/win/win_impl.rs#L43-L80 , https://github.com/rustdesk-org/rdev/blob/a361d86a8b0245f3618a9efb375c149530a6b599/src/windows/simulate.rs#L96-L112

### 3.4 viewer 와 host 의 키보드 배열이 다를 때

- 기본 전략은 두 가지다.
  - 위치 기준: scancode 를 그대로 보낸다. Microsoft 문서도 scan code 는 layout 과 무관하게 같은 물리 키를 가리킨다며 게임의 WASD 를 예로 든다. https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input
  - 문자 기준: 입력된 문자(Unicode)를 보낸다.
- RustDesk 는 세 가지 mode 를 사용자가 고르게 한다(1.2.0 이상). https://github.com/rustdesk/rustdesk/wiki/FAQ
  - Map mode: 물리 위치 기준. 로컬 QWERTY 의 q 가 원격 AZERTY 에서는 a 로 입력된다.
  - Translate mode: 로컬 layout 이 원격에서 켜져 있는 것처럼 동작. scan code 를 직접 읽는 게임에서는 문제가 생길 수 있다.
  - Legacy mode: 1.1.9 이하 버전 호환용.
- RustDesk 구현: https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/input_service.rs#L2074-L2225 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/keyboard.rs#L1459-L1530 , https://github.com/rustdesk-org/rdev/blob/a361d86a8b0245f3618a9efb375c149530a6b599/src/windows/simulate.rs#L226-L273
  - host 의 Map mode 는 `sim_rdev_rawkey_position`(scancode 주입).
  - host 의 Translate mode 는 문자열을 받으면 Shift 를 떼고 `simulate_unicode`(`KEYEVENTF_UNICODE`)로 넣는다. Ctrl, Alt, Win 이 눌린 상태면 `simulate_char` 를 쓰는데, `VkKeyScanExW` 로 host layout 에서 그 문자의 VK 와 modifier 를 찾아 키 입력으로 보낸다.
  - viewer 는 modifier 가 눌린 단축키를 VK 와 unicode 를 함께 담은 `win2win_hotkey` 로 보낸다. 그 밖에는 unicode 로 보내고, 둘 다 아니면 Map mode 로 돌아간다. dead key 는 Translate mode 에서 버린다.

### 3.5 한국어 키보드와 IME

- VK 값: `VK_HANGUL` = `VK_KANA` = 0x15, `VK_HANJA` = `VK_KANJI` = 0x19, `VK_IME_ON` = 0x16, `VK_IME_OFF` = 0x1A, `VK_PROCESSKEY` = 0xE5. https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
- HID 키 코드와 scan code 대응: `Keyboard LANG1`(HID 0x90)은 0x0072(구형 메시지 0x00F2), `LANG2`(HID 0x91)는 0x0071(구형 0x00F1). 표의 주석 6 에 "The scan code is emitted in key release event only" 라고 적혀 있다. 다른 키처럼 누름과 뗌이 짝으로 오지 않을 수 있다는 뜻이라, 한/영, 한자 키는 scancode 보다 VK 로 보내는 편이 안전해 보인다(추론). https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input
- Microsoft IME 문서 기준 한/영 전환은 오른쪽 Alt, 한자 변환은 오른쪽 Ctrl. https://learn.microsoft.com/en-us/globalization/input/korean-ime
- 한국어 키보드 종류에 따라 키 배치가 다르다. 공식 문서에는 없고 Microsoft 직원(Michael Kaplan) 블로그 아카이브에서만 찾았다, 미확인(2차 출처). 어떤 종류를 쓰는지는 `HKLM\SYSTEM\CurrentControlSet\Services\i8042prt\Parameters` 의 `LayerDriver KOR` 값으로 정해진다. http://archives.miloush.net/michkap/archive/2012/04/30/10298801.html

  | 종류 | 한/영 | 한자 |
  |---|---|---|
  | kbd101a | 오른쪽 Alt | 오른쪽 Ctrl |
  | kbd101b | 오른쪽 Ctrl | 오른쪽 Alt |
  | kbd101c | Shift+Space | Ctrl+Space |
  | kbd103 | 전용 한/영 키 | 전용 한자 키 |

- 설계에 주는 영향(추론): 같은 물리 오른쪽 Alt 가 host 설정에 따라 `VK_RMENU` 가 되기도 하고 `VK_HANGUL` 이 되기도 한다. scancode 로 보내면 host layout 이 스스로 판단하므로 자연스럽고, VK 로 보내면 viewer 나 host 중 한쪽이 변환해야 한다. rdev fork 는 host 쪽에서 변환한다. foreground layout 언어가 0x0412(한국어)이면 VK 165(`VK_RMENU`)를 `VK_HANGUL` 로 바꿔 보낸다. https://github.com/rustdesk-org/rdev/blob/a361d86a8b0245f3618a9efb375c149530a6b599/src/windows/simulate.rs#L117-L130
- RustDesk 1.4.6(Win11 에서 Win11)에서 "한/영 키를 눌러도 영문으로 입력된다" 는 이슈가 열려 있다. 한 사용자는 legacy 와 map mode 는 되고 translate mode 는 안 된다고 보고했다. 사용자 보고일 뿐 재현 확정은 아니다. https://github.com/rustdesk/rustdesk/issues/14498
- IME 조합:
  - `KEYEVENTF_UNICODE` 는 `TranslateMessage` 를 거쳐 곧바로 `WM_CHAR` 가 된다. 이 경로를 host IME 가 가로채는지는 문서에 없다(미확인). https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput
  - 설계상 선택지(추론): 키 입력(scancode 나 VK)을 보내면 host IME 가 조합하므로 host 의 IME mode 가 결과를 정한다. 완성된 음절을 Unicode 로 보내면 host IME 상태와 무관해지지만, 조합 이벤트(`WM_IME_COMPOSITION`)를 기대하는 앱에서는 다르게 동작할 수 있다.
- IME mode 동기화:
  - `ImmGetConversionStatus` 는 `IME_CMODE_NATIVE`(한글 mode), `IME_CMODE_HANJACONVERT` 값을 돌려준다. 최소 Windows XP + 동아시아 언어 지원. 대상 창의 HIMC(input context 핸들)가 있어야 한다. https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immgetconversionstatus , https://learn.microsoft.com/en-us/windows/win32/intl/ime-conversion-mode-values
  - 다른 프로세스 창의 mode 를 읽는 방법은 공식 문서에 없다. `WM_IME_CONTROL` 공식 command 목록에 `IMC_GETCONVERSIONMODE` 가 없으므로, IME 기본 창에 이 메시지를 보내 읽는 널리 알려진 방법은 미확인(2차 출처). https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-control
- RustDesk 는 host 쪽 enigo 에서 `Key::Hangul` 을 0x15, `Key::Hanja` 를 0x19 로 바꾸고, viewer 쪽에서도 Hangul, Hanja control key 를 따로 전달한다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/enigo/src/win/keycodes.rs#L56-L61 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/keyboard.rs#L1108-L1116

### 3.6 Touch 와 pen

- `InitializeTouchInjection` / `InjectTouchInput`: Windows 8 부터. `maxCount` 는 1 이상 `MAX_TOUCH_COUNT`(256) 이하. 입력은 주입하는 프로세스가 도는 session 의 desktop 으로 간다. `WM_DISPLAYCHANGE` 가 오면 진행 중인 contact 가 모두 취소된다. 호출 간격이 0.1ms 보다 짧으면 `ERROR_NOT_READY` 가 날 수 있다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-initializetouchinjection , https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-injecttouchinput
- `CreateSyntheticPointerDevice` / `InjectSyntheticPointerInput`: Windows 10 1809 부터. type 은 `PT_TOUCH` 또는 `PT_PEN`, pen 이면 contact 수가 반드시 1. `ptPixelLocation` 은 virtual screen 왼쪽 위 기준 좌표. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createsyntheticpointerdevice , https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-injectsyntheticpointerinput
- RustDesk 와 Sunshine 에서 GitHub code search 로 두 API 를 찾았지만 0건이었다. RustDesk Windows host 가 touch 를 주입하는지는 확인하지 못했다.

**RustDesk 구현 (입력):** host 입력 분기 https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/input_service.rs#L2299-L2356 , 마우스와 키 주입(enigo, `SendInput`, `MOUSEEVENTF_VIRTUALDESK`) https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/enigo/src/win/win_impl.rs , scancode, Unicode, 한/영 처리(rdev fork) https://github.com/rustdesk-org/rdev/blob/a361d86a8b0245f3618a9efb375c149530a6b599/src/windows/simulate.rs , 주입 전 input desktop 따라가기 https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L1040-L1056

## 4. viewer 쪽 단축키 가로채기

### 4.1 WH_KEYBOARD_LL 의 동작 조건

- `WH_KEYBOARD_LL` 은 global hook 으로만 설치할 수 있다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw
- hook 함수는 hook 을 설치한 thread 에서, 그 thread 로 메시지를 보내는 방식으로 불린다. 그래서 그 thread 에 message loop 가 있어야 한다. https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
- 처리 시간이 `HKCU\Control Panel\Desktop\LowLevelHooksTimeout`(ms)을 넘으면: Windows 7 이후에는 hook 이 조용히 제거되고 앱은 알 방법이 없다. Windows 10 1709 이후 이 값의 상한은 1000ms. 문서는 hook 을 전용 thread 에서 돌리고 실제 작업은 worker thread 에 넘기라고 권한다. https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
- hook 함수가 0 이 아닌 값을 돌려주면 그 키 입력은 나머지 hook chain 과 대상 창에 전달되지 않는다. 이것이 viewer 가 키를 "먹는" 방법이다. https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
- Microsoft 게임용 가이드의 LL hook 으로 Windows 키 막기 예시: Windows 2000 이상, 표준 사용자 계정에서도 동작. 창이 비활성화되면 막기를 풀도록 `WM_ACTIVATEAPP` 에서 처리. 코드 주석 "this will not block the Xbox Game Bar hotkeys (Win+G, Win+Alt+R, etc.)". https://learn.microsoft.com/en-us/windows/win32/dxtecharts/disabling-shortcut-keys-in-games
- raw input 만으로는 부족하다. `RIDEV_NOHOTKEYS` 를 켜도 원문 "the system hotkeys; for example, ALT+TAB and CTRL+ALT+DEL, are still handled". Alt+Tab 을 원격으로 넘기려면 LL hook 이 필요하다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinputdevice
- `RegisterHotKey` 는 viewer 자체 단축키용으로 불리하다. Windows 키가 들어간 조합은 OS 예약, 다른 곳에서 이미 등록한 조합이면 보통 실패, F12 는 debugger 용 예약. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey
- UIPI 제약: 낮은 권한 프로세스는 높은 권한 프로세스에 hook 을 걸 수 없고, UIAccess 프로세스만 "read input for all integrity levels using low-level hooks" 가 가능하다. 추론: 일반 viewer 는 관리자 권한 창이 foreground 일 때 그 창으로 가는 키를 LL hook 으로 못 볼 수 있다. 다만 원격 조작 중에는 viewer 창이 foreground 이므로 실제 영향은 작다(추론). https://learn.microsoft.com/en-us/previous-versions/dotnet/articles/bb625963(v=msdn.10) , https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/security-policy-settings/user-account-control-only-elevate-uiaccess-applications-that-are-installed-in-secure-locations

### 4.2 가로챌 수 없는 조합

- Ctrl+Alt+Del: Winlogon 이 초기화 단계에서 SAS 로 먼저 등록한다. 그래서 다른 앱은 이 조합에 hook 을 걸 수 없다. https://learn.microsoft.com/en-us/windows/win32/secauthn/initializing-winlogon
- Microsoft RDP client 도 같은 제약을 받는다. 원문 "CTRL+ALT+DELETE is never redirected to the remote server, even when the KeyboardHookMode property is enabled". 대신 Ctrl+Alt+End 를 쓴다(기본값 `VK_END`, Vista 이상). https://learn.microsoft.com/en-us/windows/win32/termserv/imsrdpclientadvancedsettings-hotkeyctrlaltdel , https://learn.microsoft.com/en-us/windows/win32/termserv/terminal-services-shortcut-keys
- Win+L: LL hook 으로 막을 수 없고 `DisableLockWorkstation` 정책으로만 끌 수 있다고 한다. 근거는 Microsoft Q&A 커뮤니티 답변뿐이라 미확인(2차 출처). https://learn.microsoft.com/en-us/answers/questions/1286619/blocking-windows-hotkeys-in-an-application
- Win+G, Win+Alt+R(Xbox Game Bar): 위 Microsoft 게임 가이드 예시 코드 주석에 LL hook 으로 막히지 않는다고 적혀 있다. https://learn.microsoft.com/en-us/windows/win32/dxtecharts/disabling-shortcut-keys-in-games

### 4.3 가로채기 구현 시 주의점 (오픈소스 코드 기준)

- TigerVNC viewer: LL hook 을 전용 thread 에서 돌리고, 가로챈 키는 `PostMessage` 로 viewer 창에 넘긴다. 주석에 따르면 Windows 는 키 입력을 가로채면 global keyboard state 갱신을 멈추므로 Caps Lock, Num Lock, Scroll Lock 은 통과시킨다. 가로채기 시작 전부터 눌려 있던 키는 key-up 을 통과시켜 원래 상태로 돌린다. https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/vncviewer/win32.c#L36-L200
- RustDesk 도 CapsLock, NumLock 은 통과시키고(fix #2211 주석) lock key 상태는 별도로 host 와 맞춘다. viewer 가 포커스를 잃으면 `release_remote_keys` 로 host 에 눌린 채 남은 키를 풀어 준다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/keyboard.rs#L612-L704 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/keyboard.rs#L737-L756
- RustDesk 의 rdev fork `grab` 은 `WH_KEYBOARD_LL` 과 `WH_MOUSE_LL` 을 `GetMessageA` loop 가 있는 thread 에 설치하고, callback 이 `None` 을 돌려주면 `return 1` 로 키를 먹는다. https://github.com/rustdesk-org/rdev/blob/a361d86a8b0245f3618a9efb375c149530a6b599/src/windows/grab.rs#L43-L130

### 4.4 특수 키 보내기 메뉴

- RDP 모델: `KeyboardHookMode` 로 단축키를 어디에 적용할지 정한다. 0 = 로컬, 1 = 원격, 2 = 전체 화면일 때만 원격(기본값, Vista 이상). 로컬에 남는 조합을 위해 대체 단축키 표를 둔다(Alt+Tab 대신 Alt+Page Up, Windows 키 대신 Alt+Home, Ctrl+Alt+Del 대신 Ctrl+Alt+End). https://learn.microsoft.com/en-us/windows/win32/termserv/imsrdpclientsecuredsettings-keyboardhookmode , https://learn.microsoft.com/en-us/windows/win32/termserv/terminal-services-shortcut-keys
- TeamViewer 모델: toolbar 의 Actions 메뉴에 "Send Ctrl + Alt + Del", "Lock"(Lock now, Sign out), "Reboot"(safe mode 선택 가능). "Send key combinations" 설정을 켜면 Win+E, Ctrl+P 같은 조합이 원격으로 넘어간다. macOS 에서 Windows 로 연결할 때의 조합 대응표도 공식 문서로 있다. https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/remote-session-toolbar-on-windows/ , https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/use-key-commands-in-sessions/
- RustDesk 흐름:
  - viewer 메뉴 "Insert Ctrl + Alt + Del" 은 peer 가 Linux 이거나 `pi.sasEnabled` 일 때만 보인다. "Insert Lock" 메뉴도 있다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/flutter/lib/common/widgets/toolbar.dart#L510-L540
  - legacy mode 에서 Windows peer 로 Ctrl+Alt+Del 이 키로 들어오면 `client::ctrl_alt_del()` 로 바꿔 보낸다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/keyboard.rs#L1101-L1106
  - host 는 `CtrlAltDel` 을 받으면 `send_sas` 를 부른다(물리 콘솔 session 이면 IPC 로 service 에 넘기고, 아니면 직접 처리). `LockScreen` 을 받으면 Windows 에서는 `LockWorkStation`. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/input_service.rs#L1944-L1960 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/input_service.rs#L2364-L2375
  - FAQ: Ctrl+Alt+Del 을 쓰려면 RustDesk 가 service 로 설치되어 있고 `SoftwareSASGeneration` 이 1 이나 3 이어야 한다. https://github.com/rustdesk/rustdesk/wiki/FAQ
- `SendSAS` 호출 조건은 2.7 참고. https://learn.microsoft.com/en-us/windows/win32/api/sas/nf-sas-sendsas
- `LockWorkStation`(XP 이상)은 interactive desktop 에서 도는 프로세스만 부를 수 있다. 비동기라 성공 반환값이 실제 잠금 완료를 뜻하지 않는다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-lockworkstation
- TigerVNC server 는 Ctrl+Alt 상태에서 Del scancode 가 키 입력으로 들어오면 직접 주입하지 않고 `emulateCtrlAltDel()` 로 돌린다. https://github.com/TigerVNC/tigervnc/blob/3dcfda191eb22060e13b996d8a52e7f69a41f23c/win/rfb_win32/SInput.cxx#L380-L395

**RustDesk 구현 (viewer 단축키):** 키 가로채기(rdev `grab`, `WH_KEYBOARD_LL`, `WH_MOUSE_LL`) https://github.com/rustdesk-org/rdev/blob/a361d86a8b0245f3618a9efb375c149530a6b599/src/windows/grab.rs , 가로채기 loop 와 mode 분기 https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/keyboard.rs , 특수 키 메뉴 https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/flutter/lib/common/widgets/toolbar.dart#L510-L540 , host 쪽 SAS 처리 https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L949-L1030

## 5. 해상도와 DPI

### 5.1 DPI awareness 와 좌표계

- DPI awareness 모드: https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows , https://learn.microsoft.com/en-us/windows/win32/hidpi/dpi-awareness-context
  - Unaware: 모든 모니터가 96 DPI 로 보이고 Windows 가 bitmap 을 늘려 흐릿해진다.
  - System aware: Vista 부터. 로그인 시점 primary 모니터의 DPI 하나로 고정.
  - Per-Monitor v1: 8.1 부터.
  - Per-Monitor v2 (PMv2): Windows 10 1703 부터. 각 모니터의 실제 픽셀을 보고 bitmap 스케일링을 당하지 않는다.
- 설정 방법:
  - manifest 가 권장 방법이다. `<dpiAwareness>PerMonitorV2</dpiAwareness>` 는 1607 부터 읽히고, 옛 버전에서는 `<dpiAware>` 가 fallback. https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process
  - API 로 하려면 `SetProcessDpiAwarenessContext`(1703+)를 HWND 를 만들기 전에 불러야 한다. 이미 설정돼 있으면 `ERROR_ACCESS_DENIED`. 아무것도 설정하지 않으면 기본값은 UNAWARE. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setprocessdpiawarenesscontext
- 좌표 가상화(DPI virtualization): unaware 나 system-aware 스레드가 화면 크기를 물으면 96 DPI 기준으로 바꾼 값을 받는다. 어떤 API 가 가상화된 값을 돌려주는지 충분히 문서화되지 않았다고 Microsoft 가 직접 적어 두었다. https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows
  - 그래서 host agent 는 PMv2 로 돌아야 캡처 픽셀과 입력 좌표가 맞는다(추론). 실제 구현도 그렇다. Sunshine 은 캡처 초기화 때 PMv2 를 설정하고 https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/display_base.cpp#L453-L467 , RustDesk 는 manifest 에 `PerMonitorV2, PerMonitor` 를 넣는다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/res/manifest.xml#L6-L7
- physical 좌표만 쓰는 API: `DXGI_OUTPUT_DESC.DesktopCoordinates` 는 "desktop DPI 에 의존하는 desktop 좌표". https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ns-dxgi-dxgi_output_desc . `ChangeDisplaySettingsEx` 와 `QueryDisplayConfig` 는 DPI 가상화에 참여하지 않아 항상 physical pixel 을 쓴다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-changedisplaysettingsexw , https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-querydisplayconfig
- `GetDpiForMonitor`(8.1+): 호출 프로세스의 awareness 에 따라 96, system DPI, 실제 값 중 하나를 돌려준다. dpiX 는 화면을 돌려도 항상 가로 가장자리 기준. per-monitor aware 스레드에서는 `GetDpiForWindow` 를 쓰라고 한다. https://learn.microsoft.com/en-us/windows/win32/api/shellscalingapi/nf-shellscalingapi-getdpiformonitor
- viewer 창: per-monitor 모드이면 창이 DPI 가 다른 모니터로 옮겨질 때 Windows 가 bitmap 을 늘리지 않고 `WM_DPICHANGED` 를 보낸다. 메시지에 담긴 추천 사각형(suggested rect)으로 창 크기를 맞춘다. https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows

### 5.2 viewer 좌표를 host 좌표로 바꾸기 (설계 메모, 산술이라 출처 없음)

- 원본(1:1) 모드: viewer 도 PMv2 여야 host 픽셀 1개가 viewer 픽셀 1개가 된다. 아니면 위의 bitmap stretch 로 흐려진다.
- fit 모드: `s = min(Wv/Wh, Hv/Hh)` 로 비율을 정하고 남는 여백(letterbox) offset 을 뺀다. `host_x = mon.left + (x_v - off_x) / s`.
- stretch 모드: 가로, 세로 비율을 따로 쓴다.
- 결과는 physical pixel 단위의 virtual desktop 좌표다. `SendInput` 용 0..65535 정규화는 3.2 참고.
- 모니터마다 DPI 가 달라도 host 가 PMv2 이고 캡처가 physical pixel 이면, host 쪽 좌표 변환에는 DPI 가 끼어들지 않는다(추론). DPI 는 viewer 창 크기와 UI 배율에만 영향을 준다.

### 5.3 디스플레이 변경 감지와 회전

- `WM_DISPLAYCHANGE` 는 top-level window 에만 sent 되고 나머지 창에는 posted 된다. 받으려면 host 쪽 사용자 세션 프로세스에 숨은 창이 필요하다(추론). https://learn.microsoft.com/en-us/windows/win32/gdi/wm-displaychange
- DDA 는 mode change 때 `DXGI_ERROR_ACCESS_LOST` 를 준다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe
- WGC 는 크기가 바뀌면 `Direct3D11CaptureFramePool.Recreate` 로 다시 만든다. https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture
- RustDesk 는 1초마다 display 목록을 비교해 바뀌면 client 에 알린다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/video_service.rs#L787-L793
- 회전: `DXGI_OUTPUT_DESC.Rotation` 이 `ROTATE90` 이나 `ROTATE270` 이면 가로와 세로를 바꿔 계산한다. Sunshine 이 이렇게 처리한다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/display_base.cpp#L524-L531 . DDA surface 가 회전 전 방향으로 온다는 점은 1.1 참고.

### 5.4 원격에서 해상도 바꾸기와 되돌리기 (`ChangeDisplaySettingsEx`)

- `dwFlags`: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-changedisplaysettingsexw
  - `0`: 동적으로 바꾸기만 한다.
  - `CDS_UPDATEREGISTRY`: 동적으로 바꾸고 사용자 프로필 registry 에도 저장.
  - `CDS_GLOBAL`: 모든 사용자에게 저장. `CDS_UPDATEREGISTRY` 와 함께만.
  - `CDS_FULLSCREEN`: 임시 모드. 다른 desktop 으로 갔다 와도 reset 되지 않는다.
- 되돌리기: `lpDevMode = NULL`, `dwFlags = 0` 으로 부르는 것이 동적 변경 뒤 registry 기본 모드로 돌아가는 가장 쉬운 방법. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-changedisplaysettingsexw
- 여러 모니터를 한꺼번에 바꾸려면 모니터마다 `CDS_UPDATEREGISTRY | CDS_NORESET` 로 부른 뒤 device 를 NULL 로 한 번 더 불러 적용한다. DEVMODE 는 `EnumDisplaySettings` 가 준 값을 쓰고, Windows 8 이후 대상 앱은 32bpp 미만 모드를 쓸 수 없다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-changedisplaysettingsexw
- RustDesk: `CDS_UPDATEREGISTRY | CDS_GLOBAL | CDS_RESET` 으로 registry 에 영구 저장하며 바꾼다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L3066-L3095 . 원래 해상도를 기록해 두었다가 원격 연결이 0개가 되면 `restore_resolutions` 로 되돌린다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/connection.rs#L4739-L4789 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/display_service.rs#L386-L417 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/connection.rs#L6965-L6976
  - 이 방식은 프로세스가 비정상 종료되면 바뀐 해상도가 그대로 남을 수 있다(추론). `dwFlags = 0` 동적 변경은 registry 를 건드리지 않아 복구가 쉽다(문서 근거).

### 5.5 모니터 없는 PC 용 가상 디스플레이 (IddCx)

- IDD(Indirect Display Driver)는 user-mode(UMDF) 드라이버이고, 원격 디스플레이와 가상 모니터 용도가 문서에 명시되어 있다. Session 0 에서 돌고 GDI, windowing API 를 쓰면 안 되며 desktop 이미지는 DirectX surface 로 받는다. Microsoft 샘플은 Windows-driver-samples/video/IndirectDisplay. https://learn.microsoft.com/en-us/windows-hardware/drivers/display/indirect-display-driver-model-overview
- IddCx 버전과 Windows 버전 대응. HDR 가상 디스플레이는 Windows 11 23H2 이상에서만 된다. https://learn.microsoft.com/en-us/windows-hardware/drivers/display/iddcx-versions

  | IddCx | Windows | 추가된 것 |
  |---|---|---|
  | 1.0 | Windows 10 1607 (14393) | 첫 버전 |
  | 1.2 | 1709 | |
  | 1.3 | 1803, 1809 | |
  | 1.4 | 1903, 1909 | remote session, `IddCxAdapterSetRenderAdapter` |
  | 1.5 | 2004 ~ 22H2 | |
  | 1.8 | Windows 11 21H2 | |
  | 1.9 | Windows 11 22H2 | |
  | 1.10 | Windows 11 23H2 | HDR10, WCG |

- 서명 부담은 9.4 참고.

**RustDesk 구현 (해상도, 가상 디스플레이):**

- 기본 IDD 는 Amyuni 의 "USB Mobile Monitor Virtual Display"(usbmmidd_v2, `deviceinstaller64.exe` 로 설치)이고, 자체 RustDeskIddDriver 경로도 있다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/virtual_display_manager.rs#L1-L35 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/virtual_display_manager.rs#L385-L410
- 가상 디스플레이는 Windows 10 build 19041 이상, 설치형일 때만 쓴다.
- 주석에 따르면 Amyuni 가 추가한 디스플레이는 자동으로 뺄 수 없어서, 연결 시 headless 일 때만 추가한다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/display_service.rs#L781-L800
- driver README: https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/virtual_display/dylib/README.md

## 6. 멀티 모니터

- virtual screen: 모든 모니터를 감싸는 사각형이다. primary 모니터가 원점 (0,0) 을 가지며 다른 모니터는 음수 좌표일 수 있으므로 앱은 음수 좌표를 전제로 만들어야 한다. 좌표는 signed 16-bit 범위 안에 있다. https://learn.microsoft.com/en-us/windows/win32/gdi/the-virtual-screen
  - Sunshine 은 `GetSystemMetrics(SM_CXVIRTUALSCREEN / SM_CYVIRTUALSCREEN)` 을 절대 좌표 마우스의 기준 크기로 쓴다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/display_base.cpp#L478-L480
- 모니터 열거:
  - `EnumDisplayMonitors(NULL, NULL, ...)` 는 모든 모니터의 HMONITOR 와 사각형을 준다. mirroring driver 의 보이지 않는 pseudo-monitor 가 섞일 수 있고, `SM_CMONITORS` 는 실제 모니터만 센다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaymonitors
  - DXGI 는 `EnumAdapters1` 다음 `EnumOutputs` 로 열거한다. `DXGI_OUTPUT_DESC` 에 `DeviceName`, `DesktopCoordinates`, `AttachedToDesktop`, `Rotation`, `Monitor`(HMONITOR)가 있어 GDI 쪽 목록과 연결할 수 있다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ns-dxgi-dxgi_output_desc
  - Sunshine 은 adapter 와 output 을 모두 돌며 `AttachedToDesktop` 이고 실제로 duplication 이 되는 output 만 고른다. https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/platform/windows/display_base.cpp#L490-L545
- 재연결에도 변하지 않는 모니터 ID: `QueryDisplayConfig`(Win7+)와 `DisplayConfigGetDeviceInfo(DISPLAYCONFIG_TARGET_DEVICE_NAME)` 로 `monitorDevicePath`, EDID 제조사/제품 ID, `connectorInstance` 를 얻는다. https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_target_device_name
  - `QueryDisplayConfig` 는 WDDM 드라이버에서만 되고, 호출 프로세스가 현재 desktop 에 접근할 수 없거나 원격 세션이면 `ERROR_ACCESS_DENIED`. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-querydisplayconfig
  - `\\.\DISPLAYn` 이름이 재부팅이나 재연결 때 바뀔 수 있다는 점은 미확인(2차 출처).
- 모니터별 캡처 vs virtual desktop 전체 캡처:
  - DDA 로 desktop 전체를 받으려면 활성 output 마다 duplication 을 따로 만든다. output 사이 타이밍 동기화는 없으므로 앱이 timestamp 를 보고 합쳐야 한다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput
  - 커서가 여러 output 에 걸치면 output 마다 보고하는 visible 값이 다를 수 있다. Microsoft 샘플은 `LastMouseUpdateTime` 으로 어느 쪽을 믿을지 정한다. https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api
  - 모니터가 서로 다른 GPU 에 붙어 있으면 adapter 마다 D3D device 가 필요하다(1.1 의 adapter 조건).
  - WGC 의 `CreateForMonitor` 도 HMONITOR 하나 단위다. https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createformonitor
- 세션 중 모니터 전환: 모니터 추가, 제거, mode change 가 있으면 `ACCESS_LOST` 가 나거나 열거 결과가 바뀐다. 캡처 객체는 display 단위로 버리고 다시 만들 수 있게 짜야 한다. https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe

**RustDesk 구현 (멀티 모니터):**

- display 마다 별도 VideoService 를 둔다(서비스 이름은 prefix + index). https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/video_service.rs#L296-L298
- client 는 `SwitchDisplay` / `CaptureDisplays` 메시지로 캡처할 display 를 바꾸거나 더하고 뺀다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/connection.rs#L4515-L4640
- display 는 DXGI 로 열거하고, 실패하면 GDI(`EnumDisplayDevicesW` + `EnumDisplaySettingsExW`, mirroring driver 제외)로 대신한다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/dxgi/mod.rs#L773-L940 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/common/dxgi.rs#L125-L200

## 7. 클립보드

### 7.1 변경 감지와 접근

- `AddClipboardFormatListener` 로 창을 등록하면 클립보드 내용이 바뀔 때마다 그 창에 `WM_CLIPBOARDUPDATE` 가 post 된다. 최소 Windows Vista / Server 2008. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-addclipboardformatlistener
- `GetClipboardSequenceNumber` 는 window station 마다 따로 관리되는 일련번호다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclipboardsequencenumber
  - 내용이 바뀌거나 클립보드가 비워질 때 올라간다.
  - delayed rendering(데이터를 실제로 요청받을 때 만드는 방식)을 쓰면 렌더링되기 전까지는 올라가지 않는다.
  - `WINSTA_ACCESSCLIPBOARD` 권한이 없으면 0 을 돌려준다.
  - 추론: 클립보드가 window station 마다 따로 있으므로 session 0 서비스는 사용자 클립보드를 다룰 수 없다. 사용자 세션 안의 helper 가 맡아야 한다.
- 한 번에 창 하나만 클립보드를 열 수 있다. 다른 창이 열고 있으면 `OpenClipboard` 는 실패하는데, 어떤 에러 코드가 나오는지는 문서에 없다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard , https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-operations
  - RustDesk 는 이 실패를 흔한 일로 보고 33ms 간격으로 3회 재시도한다. 주석에는 `ERROR_CLIPBOARD_NOT_OPEN`(1418)을 관찰했다고 적혀 있다. https://github.com/rustdesk/rustdesk/blob/master/src/clipboard.rs
- `OpenClipboard(NULL)` 뒤에 `EmptyClipboard` 를 부르면 owner 가 NULL 이 되어 `SetClipboardData` 가 실패한다. 쓰기용으로 열 때는 창 handle 을 넘겨야 한다. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard

### 7.2 포맷

- `CF_UNICODETEXT`: 시스템이 `CF_TEXT`, `CF_OEMTEXT` 와 자동으로 서로 변환한다(synthesized format). 여러 포맷을 올릴 때는 정보가 가장 많은 포맷부터 올린다. https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats
- `"HTML Format"`(CF_HTML): https://learn.microsoft.com/en-us/windows/win32/dataxchg/html-clipboard-format
  - `RegisterClipboardFormat` 에 넘기는 이름은 `"HTML Format"`.
  - 인코딩은 항상 UTF-8. 보통 UTF-16 을 쓰는 Windows API 와 다른 예외다.
  - 헤더 필수 항목: `Version`, `StartHTML`, `EndHTML`, `StartFragment`, `EndFragment`. 선택: `StartSelection`, `EndSelection`. offset 은 모두 byte 단위.
  - fragment 는 `<!--StartFragment-->` 와 `<!--EndFragment-->` 로 감싼다.
  - Windows 10 20H2 부터 `Version:1.0` 을 쓴다.
- `CF_DIB` / `CF_DIBV5`: 시스템이 `CF_BITMAP`, `CF_DIB`, `CF_DIBV5` 를 서로 변환해 준다. 비트맵은 `CF_DIB` 나 `CF_DIBV5` 로 올리라고 권장한다. `CF_DIBV5` 에 색 공간 정보가 있으면 요청받을 때 sRGB 로 변환된다. https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats
- `"PNG"`(registered format): Win32 문서에는 정의가 없다. Office 와 브라우저가 이 포맷을 쓴다는 내용은 Mozilla bugzilla 에만 있어 미확인(2차 출처). https://bugzilla.mozilla.org/show_bug.cgi?id=1717306
  - 구현 예: arboard(Rust 클립보드 라이브러리)는 이미지를 쓸 때 `CF_DIBV5` 와 `"PNG"` 를 함께 올리고, 읽을 때는 PNG 를 먼저 본다. https://github.com/1Password/arboard/blob/master/src/platform/windows.rs
- `CF_HDROP`: `DROPFILES` 구조에 전체 경로를 담고, 경로 배열은 null 두 개로 끝난다. https://learn.microsoft.com/en-us/windows/win32/shell/clipboard
  - 추론: 경로만 담기 때문에 원격 PC 에서는 그 경로에 파일이 없다. 원격 파일 붙여넣기에는 쓸 수 없다.
- `CFSTR_FILEDESCRIPTORW` + `CFSTR_FILECONTENTS`: 가상 파일(디스크에 없는 파일)을 전달하는 포맷. https://learn.microsoft.com/en-us/windows/win32/shell/clipboard
  - `FILEGROUPDESCRIPTOR` 와 `FILEDESCRIPTOR` 배열로 파일 목록을 알린다.
  - 받는 쪽은 `FORMATETC.lindex` 에 파일 번호를 넣어 `CFSTR_FILECONTENTS` 를 요청한다.
  - 보통 `TYMED_ISTREAM`(`IStream`)으로 받기 때문에 파일 전체를 메모리에 올리지 않는다.

### 7.3 원격 파일 붙여넣기와 delayed rendering

- `SetClipboardData(fmt, NULL)` 로 올리면 데이터가 실제로 필요한 순간에 owner 창이 `WM_RENDERFORMAT` 을 받는다. 이 메시지를 처리하는 중에는 `OpenClipboard` 를 부르면 안 된다. owner 창이 사라지기 전에는 `WM_RENDERALLFORMATS` 를 받고, 여기서 렌더링하지 않은 포맷은 사라진다. https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-operations
- 같은 문서의 경고: 렌더링은 window message 처리 안에서 일어나므로 오래 걸리면 앱이 "응답 없음" 처럼 보인다. 추론: 네트워크 너머에서 데이터를 받아와야 하는 원격 붙여넣기가 바로 이 경우다. https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-operations
- `OleSetClipboard(IDataObject*)` 는 모든 포맷을 delayed rendering 으로 제공한다. 내부 OLE 창이 `WM_RENDERFORMAT` 을 받아 `IDataObject` 에 넘긴다. 앱이 끝난 뒤에도 데이터를 남기려면 `OleFlushClipboard`. 최소 Windows 2000. https://learn.microsoft.com/en-us/windows/win32/api/ole2/nf-ole2-olesetclipboard
- RDP 는 같은 문제를 [MS-RDPECLIP] 에서 정의한다(Format List, File Contents Request/Response 등). 우리 프로토콜 메시지 설계에 참고할 수 있다. https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-rdpeclip/fb9b7e0b-6db4-41c2-b83c-f889c1ee7688
- RustDesk 의 Windows 파일 복사-붙여넣기는 FreeRDP 의 `wf_cliprdr.c`(Apache-2.0)를 가져온 것이다. `OleInitialize` 뒤 `OleSetClipboard` 로 `IDataObject` 를 올리고, `CFSTR_FILEDESCRIPTORW` 와 `CFSTR_FILECONTENTS` 를 제공하며, 파일 내용은 자체 `IStream` 구현(`CliprdrStream_Read`)이 원격에서 끌어온다. 메시지 종류는 `FormatList`, `FormatListResponse`, `FileContentsRequest`, `FileContentsResponse`. https://github.com/rustdesk/rustdesk/blob/master/libs/clipboard/src/windows/wf_cliprdr.c , https://github.com/rustdesk/rustdesk/blob/master/libs/clipboard/src/lib.rs

### 7.4 echo loop 방지

echo loop 는 원격에서 받아 로컬에 쓴 내용을 다시 "변경" 으로 감지해 원격으로 돌려보내는 순환을 말한다.

- 방법 1, 내가 쓴 데이터에 표식 포맷을 함께 올린다. RustDesk 는 `"dyn.com.rustdesk.owner"` 포맷에 표식 바이트를 넣는다(host 가 쓰면 `0b01`, client 가 쓰면 `0b10`). 읽을 때 이 표식이 있으면 동기화하지 않고, 빈 텍스트도 보내지 않는다. https://github.com/rustdesk/rustdesk/blob/master/src/clipboard.rs
- 방법 2, 쓴 직후의 `GetClipboardSequenceNumber` 값을 기억해 두고 `WM_CLIPBOARDUPDATE` 때 같은 값이면 무시한다. sequence number 정의에서 이끌어 낸 추론이며, delayed rendering 을 쓰면 번호가 렌더링 시점에 올라가므로 주의. https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclipboardsequencenumber
- RustDesk 가 쓰는 변경 감지 라이브러리 clipboard-master 는 Windows 에서 `AddClipboardFormatListener` 와 `WM_CLIPBOARDUPDATE` 를 쓴다. https://github.com/rustdesk-org/clipboard-master/blob/master/src/master/win32.rs

### 7.5 민감 데이터: 클립보드 기록과 cloud clipboard 에서 빼기

Cloud Clipboard(Windows 10 1809 도입)는 최근 항목 기록(Win+V)을 남기고 기기끼리 동기화한다. 아래 세 registered format 으로 막거나 허용한다. 모두 `RegisterClipboardFormat` 으로 ID 를 받아 쓴다. https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats

| 포맷 | 넣는 값 | 효과 |
|---|---|---|
| `ExcludeClipboardContentFromMonitorProcessing` | 아무 데이터 | 모든 포맷이 기록과 동기화 양쪽에서 빠진다 |
| `CanIncludeInClipboardHistory` | DWORD 0 또는 1 | 0 이면 기록에서 빠지고, 1 이면 기록에 넣으라는 명시적 요청. 동기화에는 영향 없음 |
| `CanUploadToCloudClipboard` | DWORD 0 또는 1 | 0 이면 다른 기기로 동기화하지 않고, 1 이면 동기화를 명시적으로 요청. 로컬 기록에는 영향 없음 |

- arboard 는 이 세 포맷을 `exclude_from_monitoring`, `exclude_from_cloud`, `exclude_from_history` 옵션으로 제공한다. https://github.com/rustdesk-org/arboard/blob/master/src/platform/windows.rs
- RustDesk `clipboard.rs` 에서는 이 옵션을 쓰는 곳을 찾지 못했다(검색 기준). https://github.com/rustdesk/rustdesk/blob/master/src/clipboard.rs

### 7.6 크기 제한

- Win32 클립보드 문서에는 크기 상한이 없고, 메모리를 `GlobalAlloc(GMEM_MOVEABLE)` 로 잡으라는 규칙만 있다. 상한은 앱이 정해야 한다. https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-operations
- 같은 문서의 delayed rendering 가이드: 이미 완성된 100 KiB 이하 데이터는 delayed rendering 의 추가 비용이 그냥 복사하는 비용보다 컸다는 측정이 있고, 텍스트 한 포맷만 다루고 4 KiB 이하면 바로 올리라고 권한다. https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-operations
- RustDesk 는 클립보드 데이터를 zstd 로 압축해 보내되 압축 결과가 원본보다 작을 때만 압축본을 쓴다. 압축을 풀 때 상한은 `MAX_DECOMPRESSED_SIZE = 256 MiB`. 별도의 전송 크기 상한은 찾지 못했다. https://github.com/rustdesk/rustdesk/blob/master/src/clipboard.rs , https://github.com/rustdesk/hbb_common/blob/main/src/compress.rs

**RustDesk 구현 (클립보드):** 동기화 포맷은 Text, Html, Rtf, ImageRgba, ImagePng, ImageSvg, `"XML Spreadsheet"`. 포맷 목록과 owner 표식, 재시도는 https://github.com/rustdesk/rustdesk/blob/master/src/clipboard.rs , 서비스 loop 는 https://github.com/rustdesk/rustdesk/blob/master/src/server/clipboard_service.rs , Windows 파일 복사-붙여넣기는 https://github.com/rustdesk/rustdesk/blob/master/libs/clipboard/src/windows/wf_cliprdr.c , 메시지 변환은 https://github.com/rustdesk/rustdesk/blob/master/src/clipboard_file.rs

## 8. 화면용 인코딩 / 디코딩

### 8.1 Windows 내장: Media Foundation (MF)

- H.264 encoder MFT: 최소 Windows 7. Baseline 과 Main profile 지원, High profile 은 Windows 8 부터. 입력은 I420, IYUV, NV12, YUY2, YV12 뿐이고 RGB 는 받지 않는다(추론: 캡처 결과 BGRA 를 NV12 로 바꾸는 단계가 필요하다). CBR 과 peak-constrained VBR 은 Windows 8 부터. B-frame 기본값 0. `CODECAPI_AVEncVideoForceKeyFrame` 으로 다음 프레임을 key frame 으로 만들 수 있다. 인증된 hardware encoder 가 있으면 보통 내장 encoder 대신 그것이 쓰인다. https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-encoder
- `CODECAPI_AVLowLatencyMode`: 최소 Windows 8. encoder 는 프레임 재정렬로 지연을 만들면 안 되고 입력 1장에 출력 1장을 내야 한다. https://learn.microsoft.com/en-us/windows/win32/medfound/codecapi-avlowlatencymode
  - MF H.264 encoder 는 기본이 slice 병렬 처리라 지연이 낮다. 이 값을 FALSE 로 두면 여러 프레임을 병렬 처리해 지연이 늘어난다. https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-encoder
- Hardware MFT: `MFTEnumEx` 의 `MFT_ENUM_FLAG_HARDWARE` 로 찾는 hardware MFT 는 항상 비동기로 처리한다(Windows 7+). 앱은 `METransformNeedInput` / `METransformHaveOutput` 이벤트 방식을 다뤄야 한다. https://learn.microsoft.com/en-us/windows/win32/api/mfapi/nf-mfapi-mftenumex , https://learn.microsoft.com/en-us/windows/win32/medfound/hardware-mfts
- HEVC:
  - encoder MFT: 최소 Windows 10, Main 4:2:0 8-bit(`eAVEncH265VProfile_Main_420_8`)만. https://learn.microsoft.com/en-us/windows/win32/medfound/h-265---hevc-video-encoder
  - decoder MFT: 최소 Windows 10, Main 과 Main10, 4:2:0 만. GPU 가속(DXVA)은 DX11/DX12 방식만. https://learn.microsoft.com/en-us/windows/win32/medfound/h-265---hevc-video-decoder
  - Microsoft Store 의 "HEVC Video Extensions"(미국 스토어 $0.99)가 하드웨어 없는 기기에 소프트웨어 decode/encode 를 제공한다. 추론: HEVC 가 모든 PC 에서 기본으로 되는 것은 아니다. https://apps.microsoft.com/detail/9nmzlz57r3t7
- H.264 decoder MFT: 최소 Windows 7. Baseline, Main, High 를 level 5.1 까지. 4:2:0 과 monochrome 만 지원. 최대 4096x2304 이지만 GPU 가속이 보장되는 범위는 1920x1088 까지. https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-decoder
- AV1: Store 의 "AV1 Video Extension"(무료)이 decode 를 제공한다. https://apps.microsoft.com/detail/9mvzqvxjbq9v . AV1 encoder MFT 에 대한 Microsoft Learn 문서는 찾지 못했다, 미확인(2차 출처). https://www.phoronix.com/news/Microsoft-AV1-Encode-DX12-HMFT
- D3D11 하드웨어 디코딩: `IMFDXGIDeviceManager` 를 `MFT_MESSAGE_SET_D3D_MANAGER` 로 decoder 에 넘긴다. GPU 가 해당 구성을 지원하지 않으면 `MF_E_UNSUPPORTED_D3D_TYPE` 을 돌려주고 소프트웨어 decode 로 넘어간다. https://learn.microsoft.com/en-us/windows/win32/medfound/supporting-direct3d-11-video-decoding-in-media-foundation

### 8.2 GPU 제조사 SDK

**NVIDIA NVENC (Video Codec SDK 13.1)**

- Windows 10 이상 지원, 비동기 encode 도 Windows 10 이상. tuning info 는 High quality, Low latency, Ultra-low latency, Lossless 네 가지이고 각각 preset P1(가장 빠름)~P7(가장 느림). https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-video-encoder-api-prog-guide/index.html
- NVIDIA 권장 저지연 설정(game streaming, 화상회의용): Ultra-low latency 또는 Low latency tuning, CBR, Multi Pass quarter/full, 아주 작은 VBV(예: 한 프레임 = bitrate/framerate), B-frame 없음, 무한 GOP, AQ. 오류 복구용으로 LTR(long-term reference), intra refresh, non-reference P, force IDR. https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-video-encoder-api-prog-guide/index.html
- Reference picture invalidation: 뷰어가 깨진 프레임을 알려 주면 서버가 `NvEncInvalidateRefFrames` 로 그 프레임을 참조 대상에서 뺀다(`inputTimeStamp` 로 식별). 뷰어에서 서버로 가는 역방향 채널이 필요하다. 역방향 채널이 없거나 무한 GOP 를 쓸 때는 intra refresh 가 대안. `NvEncReconfigureEncoder` 로 세션을 다시 만들지 않고 bitrate, frame rate, 해상도를 바꿀 수 있다. https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-video-encoder-api-prog-guide/index.html
- H.264 4:4:4 encode 지원(CAVLC 로만), HEVC 4:4:4 encode 지원, ARGB 입력 직접 encode 가능. https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-application-note/index.html
- 동시 encode 세션 한도: GeForce 같은 "non-qualified" GPU 는 시스템 전체 합계 12개. qualified GPU 는 시스템 자원이 허락하는 만큼. 지원표에서도 GeForce 는 12, RTX A6000 은 "Unrestricted". https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-application-note/index.html , https://developer.nvidia.com/video-encode-and-decode-gpu-support-matrix-new
- 세대별 지원(지원표 HTML 을 직접 파싱해 확인). https://developer.nvidia.com/video-encode-and-decode-gpu-support-matrix-new

| 항목 | 지원 세대 |
|---|---|
| H.264 4:4:4 / HEVC 4:4:4 encode | Pascal(GTX 1080)부터 |
| AV1 encode | Ada(RTX 40xx), Blackwell. Ampere, Turing 은 없음 |
| HEVC 4:4:4 decode (NVDEC) | Turing(RTX 20xx)부터 |
| H.264 decode (NVDEC) | 4:2:0, 4:2:2 열만 있고 4:4:4 열은 없음 |
| AV1 decode (NVDEC) | Ampere 부터 |

- Sunshine(GPL-3.0)의 실제 NVENC 설정: `NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY`, 무한 GOP(`NVENC_INFINITE_GOPLENGTH`), `frameIntervalP = 1`(B-frame 없음), CBR, `vbvBufferSize = bitrate / framerate`, intra refresh 선택 가능, `NvEncInvalidateRefFrames` 사용, 4:4:4 는 `NV_ENC_H264_PROFILE_HIGH_444` 와 `chromaFormatIDC = 3`. https://github.com/LizardByte/Sunshine/blob/master/src/nvenc/nvenc_base.cpp

**AMD AMF**

- H.264, HEVC, AV1 모두 usage 로 `ULTRA_LOW_LATENCY`, `LOW_LATENCY`, `LOW_LATENCY_HIGH_QUALITY` 등을 제공하고 `LOWLATENCY_MODE` 속성과 intra refresh 설정이 있다. H.264 에서는 LTR 과 B-frame 을 동시에 쓸 수 없다. https://github.com/GPUOpen-LibrariesAndSDKs/AMF/blob/master/amf/doc/AMF_Video_Encode_API.md , https://github.com/GPUOpen-LibrariesAndSDKs/AMF/blob/master/amf/doc/AMF_Video_Encode_HEVC_API.md , https://github.com/GPUOpen-LibrariesAndSDKs/AMF/blob/master/amf/doc/AMF_Video_Encode_AV1_API.md
- 4:4:4 encode 설정은 위 세 문서에서 찾지 못했다. 지원 여부 미확인.
- AMF LICENSE 에는 "AMD 는 AVC/HEVC 등 코덱 IP 라이선스를 주지 않으며, 로열티는 사용자가 낸다" 는 취지의 고지가 있다. https://github.com/GPUOpen-LibrariesAndSDKs/AMF/blob/master/LICENSE.txt

**Intel QSV / oneVPL**

- Linux 용 `intel/media-driver` 기능표 기준: HEVC 8-bit hardware encode 는 ICL(Ice Lake) 세대부터 AYUV 입력(4:4:4)을 받는다. AV1 encode 는 DG2/ATSM, MTL(Meteor Lake) 이후. Windows 드라이버에서도 같은지는 미확인. https://github.com/intel/media-driver/blob/master/docs/media_features.md

### 8.3 소프트웨어 코덱과 라이선스

| 라이브러리 | 용도 | 코드 라이선스 | 특허 조건 | 출처 |
|---|---|---|---|---|
| x264 | H.264 enc | GPL v2 or later. 상용 라이선스 별도(licensing@x264.com) | H.264 특허 풀은 따로 | https://www.videolan.org/developers/x264.html , https://code.videolan.org/videolan/x264/-/blob/master/x264.c |
| OpenH264 | H.264 enc/dec | BSD 2-Clause | 아래 별도 설명 | https://www.openh264.org/BINARY_LICENSE.txt , https://www.openh264.org/faq.html |
| libvpx | VP8/VP9 | BSD 3-Clause | Google "Additional IP Rights Grant"(로열티 없는 특허 허락) | https://chromium.googlesource.com/webm/libvpx/+/refs/heads/main/LICENSE , https://chromium.googlesource.com/webm/libvpx/+/refs/heads/main/PATENTS |
| libaom | AV1 enc/dec | BSD 2-Clause | AOM Patent License 1.0 | https://aomedia.googlesource.com/aom/+/refs/heads/main/LICENSE , https://aomedia.googlesource.com/aom/+/refs/heads/main/PATENTS |
| SVT-AV1 | AV1 enc | BSD 3-Clause Clear | AOM Patent License 1.0 | https://github.com/AOMediaCodec/SVT-AV1/blob/main/LICENSE.md , https://github.com/AOMediaCodec/SVT-AV1/blob/main/PATENTS.md |
| dav1d | AV1 dec | BSD 2-Clause | 따로 없음 | https://code.videolan.org/videolan/dav1d/-/blob/master/COPYING |

- 참고 구현의 라이선스: RustDesk 는 AGPL-3.0, Sunshine 은 GPL-3.0(GitHub API 의 license 필드로 확인). 설계를 읽고 참고할 수는 있지만 코드를 가져오면 해당 라이선스 의무가 따라온다. https://github.com/rustdesk/rustdesk/blob/master/LICENCE , https://github.com/LizardByte/Sunshine/blob/master/LICENSE
- OpenH264 특허 조건: Cisco 가 MPEG LA(현 Via LA) 로열티를 대신 내 주는 경우는 Cisco 가 배포한 binary 를 쓸 때뿐이다. 조건은 (1) binary 를 앱에 묶어 배포하지 않고 사용자 기기에서 따로 내려받을 것, (2) 사용자가 켜고 끌 수 있을 것, (3) "OpenH264 Video Codec provided by Cisco Systems, Inc." 문구 표시. 소스에서 직접 빌드하면 로열티는 스스로 낸다. FAQ 기준으로 Baseline profile 만 지원한다. https://www.openh264.org/BINARY_LICENSE.txt , https://www.openh264.org/faq.html
- 특허 풀:
  - H.264 는 Via LA 가 관리한다. PC 코덱 기준 로열티는 연 10만 대까지 무료, 500만 대까지 대당 $0.20, 그 이상 대당 $0.10, 기업당 연간 상한 $9.75M. https://www.via-la.com/licensing-programs/avc-h-264/
  - HEVC 는 Access Advance(HEVC Advance)가 기기와 소프트웨어를 모두 대상으로 한다. 세부 요율은 별도 페이지. https://accessadvance.com/licensing-programs/hevc-advance/
- 저지연 설정 근거:
  - x264 `--tune zerolatency` 는 lookahead 0, sync_lookahead 0, bframe 0, sliced_threads 1, vfr_input 0, mb_tree 0 으로 설정한다. https://code.videolan.org/videolan/x264/-/blob/master/common/base.c
  - libvpx: `VP9E_SET_TUNE_CONTENT = VP9E_CONTENT_SCREEN`. https://chromium.googlesource.com/webm/libvpx/+/refs/heads/main/vpx/vp8cx.h
  - libaom: `AV1E_SET_TUNE_CONTENT = AOM_CONTENT_SCREEN`, 화면 전용 도구로 palette(`AV1E_SET_ENABLE_PALETTE`)와 intra block copy(`AV1E_SET_ENABLE_INTRABC`). https://aomedia.googlesource.com/aom/+/refs/heads/main/aom/aomcx.h

### 8.4 글자 선명도(YUV 4:4:4) 선택지

4:4:4 는 색 정보를 줄이지 않는 방식이라 화면 글자가 번지지 않는다. host 의 encode 와 뷰어의 decode 가 둘 다 되어야 쓸 수 있다.

| 경로 | encode 가능한 곳 | decode 가능한 곳 | 판단 |
|---|---|---|---|
| H.264 4:4:4 | NVENC(Pascal+) | MF H.264 decoder 불가(4:2:0 만). NVDEC 표에도 4:4:4 열 없음 | 뷰어에서 소프트웨어 decode 필요. 어떤 decoder 가 되는지는 미확인 |
| HEVC 4:4:4 | NVENC(Pascal+). Intel ICL+(Linux 문서 기준) | NVDEC Turing+. MF HEVC decoder 는 4:2:0 만 | NVIDIA 끼리 연결할 때 현실적 |
| VP9 / AV1 profile 1 | libvpx / libaom (소프트웨어) | libvpx / libaom / dav1d | RustDesk 가 쓰는 경로. CPU 부하 큼 |

- 표 출처: https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-decoder , https://learn.microsoft.com/en-us/windows/win32/medfound/h-265---hevc-video-decoder , https://developer.nvidia.com/video-encode-and-decode-gpu-support-matrix-new
- RustDesk 는 4:4:4(I444)를 VP9 와 AV1 에서만 켠다. 연결된 모든 뷰어가 I444 를 원하고 디코딩할 수 있을 때만 켜고, 하드웨어 경로에서는 항상 끈다. https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/codec.rs

### 8.5 대역폭 적응

- RustDesk `video_qos.rs`: FPS 는 15 로 시작해 1~120 범위에서 움직이고, 자동으로 내릴 때 하한은 5. 지연 기준 150ms, bitrate 비율은 3초마다 조정. 지연 측정용 probe(TestDelay)를 영상과 같은 길로 보내므로 순수 왕복 시간이 아니라 앞에 쌓인 대기열 지연을 잰다. 혼잡하면 bitrate 를 먼저 줄이고 FPS 는 그다음에 줄인다. https://github.com/rustdesk/rustdesk/blob/master/src/server/video_qos.rs
- encoder 쪽 조정 수단: NVENC 는 `NvEncReconfigureEncoder` 로 세션을 유지한 채 바꾼다. https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-video-encoder-api-prog-guide/index.html . MF H.264 encoder 는 Windows 8 부터 encode 도중에 `CODECAPI_AVEncCommonMeanBitRate` 를 바꿀 수 있다. https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-encoder

**RustDesk 구현 (인코딩):**

- 기본 코덱은 VP9. 연결된 모든 뷰어가 디코딩할 수 있는 코덱 중에서 고른다. 하드웨어 encoder 를 만들지 못하면 VP9 로 되돌아간다. https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/codec.rs
- VP8/VP9 는 libvpx, CBR 과 `VPX_DL_REALTIME`. https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/vpxcodec.rs
- AV1 은 libaom, `AOM_USAGE_REALTIME`, CBR, lag 0, `AOM_CONTENT_SCREEN`. https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/aom.rs
- H.264/H.265 는 hwcodec 을 거친다. encode 는 FFmpeg 의 nvenc, amf, qsv, decode 는 d3d11va. NVIDIA 에서는 CUDA context 대신 D3D11 을 쓰는데, README 는 CUDA context 로 시스템이 멈춘 사례를 이유로 든다. https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/hwcodec.rs , https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/vram.rs , https://github.com/rustdesk-org/hwcodec

## 9. 배포 제약

### 9.1 코드 서명과 SmartScreen

- SmartScreen 은 publisher reputation 과 file hash reputation 두 신호를 본다. 서명한 새 바이너리도 reputation 이 쌓일 때까지 경고가 뜬다. 정해진 기준은 없고 "several weeks and hundreds of clean installs" 가 걸릴 수 있으며, 일반 개발자용 수동 제출 경로는 없다. https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
- EV 인증서로 SmartScreen 을 바로 통과하던 동작은 2024년에 없어졌고, 지금은 OV 와 똑같이 취급된다. https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- 서명 방식 비교(문서 날짜 2026-08-29). https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options

| 방식 | 비용 | SmartScreen | 비고 |
|---|---|---|---|
| Store MSIX | 무료 | 경고 없음 | Microsoft 가 다시 서명 |
| Store MSI/EXE | CA 마다 다름 | Store 설치 시 경고 없음 | Trusted Root CA 인증서로 직접 서명 필요 |
| Azure Artifact Signing (옛 Trusted Signing) | 월 약 $9.99 | reputation 누적 필요 | 조직은 USA, Canada, EU, UK / 개인은 USA, Canada 만 가능 |
| OV 인증서 | 연 $150~300 | reputation 누적 필요 | 2023년 6월부터 HSM 또는 하드웨어 토큰 필수 |
| EV 인증서 | 연 $400 이상 | OV 와 같음 | SmartScreen 목적으로는 권장하지 않음 |

  - 같은 문서는 OV 인증서를 "조직이 USA, Canada, EU, UK 밖에 있어 Artifact Signing 을 쓸 수 없을 때" 의 선택지로 명시한다. 한국 조직이라면 남는 선택지는 OV 인증서 또는 Store 배포다.
- 서명 신원을 계속 같은 것으로 써야 reputation 이 이어진다. PUA(Potentially Unwanted Application) 행동을 하는 파일에 서명하면 인증서 자체가 나쁜 reputation 을 얻을 수 있다. https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
- Windows 11 의 Smart App Control 은 좋은 reputation 이 없는 unsigned 실행 파일을 인터넷 다운로드 여부와 관계없이 막는다. https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
- 오픈소스 프로젝트는 SignPath Foundation 에서 무료 서명을 받을 수 있다. https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options

### 9.2 Defender 와 원격 접근 도구 분류

- Microsoft 기준에서 "Lack of choice" 에 해당하면 unwanted software 로 분류된다. 해당 행동: 활성 상태를 분명히 보여주지 않거나 존재를 숨김, OS 나 브라우저의 동의 창을 우회함, 알림과 동의 없이 데이터를 저장하거나 전송함(또는 그 활동을 숨기는 옵션 제공). 제거는 Add or Remove Programs 같은 표준 방식이어야 한다. PUA 카테고리에는 "Evasion software" 와 "Poor industry reputation" 이 있다. https://learn.microsoft.com/en-us/defender-xdr/criteria
- 과거에는 `RemoteAccess:Win32/...` 탐지명이 있었다(예: RemotelyAnywhere). 지금은 definition 1.147.1889.0 부터 "no longer detects" 로 바뀌었다. 원격 관리 기능 자체가 아니라 행동 기준으로 판단한다는 뜻이다. https://www.microsoft.com/en-us/wdsi/threats/malware-encyclopedia-description?name=RemoteAccess:Win32/RemotelyAnywhere
- 공격 그룹 Storm-1811 이 Quick Assist 를 악용해 ScreenConnect, NetSupport 같은 RMM(Remote Monitoring and Management) 도구를 배포한 사례가 있다. Microsoft 는 "consider blocking or uninstalling Quick Assist and other remote monitoring and management tools if these tools are not in use" 라고 권고한다. 기업 환경에서는 우리 도구도 차단 목록에 오를 수 있다. https://www.microsoft.com/en-us/security/blog/2024/05/15/threat-actors-misusing-quick-assist-in-social-engineering-attacks-leading-to-ransomware/

### 9.3 서비스 설치

- `CreateService` 에는 `SC_MANAGER_CREATE_SERVICE` 권한(사실상 관리자)이 필요하다. `lpServiceStartName` 이 NULL 이면 LocalSystem 으로 실행되고, `SERVICE_WIN32_OWN_PROCESS` 는 독립 프로세스로 실행된다. 실행 경로에 공백이 있으면 따옴표로 감싸야 한다. 최소 XP. https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-createservicew
- 서비스가 `SERVICE_STOPPED` 를 보고하지 않고 종료되면 실패로 본다. N번째 실패에는 `lpsaActions[N-1]` 이 실행되고, `dwResetPeriod` 가 지나면 실패 횟수가 초기화된다. 설정은 `ChangeServiceConfig2`. https://learn.microsoft.com/en-us/windows/win32/api/winsvc/ns-winsvc-service_failure_actionsw
- MSIX 로 서비스를 배포하는 경우: https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-desktop6-service , https://learn.microsoft.com/en-us/windows/msix/packaging-tool/convert-an-installer-with-services
  - `desktop6:Service` 의 `StartAccount` 로 localSystem 을 지정할 수 있다.
  - restricted capability 인 `packagedServices` 또는 `localSystemServices` 가 필요하다.
  - 설치에 관리자 권한이 필요하고, 패키지 밖 서비스에 의존하는 서비스는 지원하지 않는다.
  - 최소 OS 가 문서마다 다르다. 요소 문서는 Win10 1903(18362), Packaging Tool 문서는 Win10 2004.
- RustDesk 설치는 `sc create {app} binpath= "\"exe\" --service" start= auto` 와 `netsh` 방화벽 규칙을 쓴다. 검색한 범위에서는 `sc failure` 설정을 찾지 못했고, 대신 서비스 루프가 helper 종료를 감지해 다시 띄운다. https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L3939

### 9.4 가상 디스플레이 드라이버(IddCx)를 쓸 때의 서명

- IDD(Indirect Display Driver)는 UMDF(user-mode driver framework) 드라이버이고 session 0 에서 돈다. https://learn.microsoft.com/en-us/windows-hardware/drivers/display/indirect-display-driver-model-overview
- Win10 1607 부터 새 kernel-mode 드라이버는 Dev Portal 서명이 있어야 로드된다. dashboard 계정을 만들려면 EV 인증서가 필요하고, PnP 설치 서명 요건도 따로 충족해야 한다. https://learn.microsoft.com/en-us/windows-hardware/drivers/install/kernel-mode-code-signing-policy--windows-vista-and-later-
- attestation 서명의 조건(문서 날짜 2026-03). https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/driver-signing-offerings
  - 문서는 "For testing purposes only" 라고 명시한다.
  - Windows 10 Desktop 이상에서만 유효하다.
  - kernel-mode 와 user-mode 드라이버 모두 지원한다.
  - EV 인증서가 필요하다.
  - Windows Update 로 일반 사용자에게 배포할 수 없다.
  - Windows Server 2016 이상은 HLK(Hardware Lab Kit) 를 통과한 서명만 로드한다.

## 10. macOS (후속 단계 요약)

developer.apple.com 문서 내용과 최소 버전은 `https://developer.apple.com/tutorials/data/documentation/<path>.json` endpoint 로 확인했고, 인용 URL 은 사람이 읽는 `/documentation/<path>` 주소로 적었다. Apple 문서에는 macOS 27, 27.2 항목이 이미 올라와 있다(조사일 기준).

### 10.1 화면 캡처: ScreenCaptureKit 과 이전 API

- ScreenCaptureKit 은 macOS 12.3 부터. 영상은 `CMSampleBuffer` 로 오고, 권한을 받으려면 Info.plist 에 `NSScreenCaptureUsageDescription` 이 필요하다. https://developer.apple.com/documentation/screencapturekit
- 구성: `SCShareableContent` 로 대상을 고르고 `SCContentFilter` 로 범위를 정한다. 디스플레이 단위로 캡처하면서 특정 창을 빼려면 `init(display:excludingWindows:)`(12.3). 출력 설정은 `SCStreamConfiguration`(12.3). https://developer.apple.com/documentation/screencapturekit/sccontentfilter/init(display:excludingwindows:) , https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration
- `SCStreamConfiguration` 주요 속성:
  - `showsCursor`: 기본 true, 커서를 영상에 합성할지만 정한다. https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration/showscursor
  - `pixelFormat`: `BGRA`, `l10r`(ARGB2101010), `420v`, `420f`. https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration/pixelformat
  - `minimumFrameInterval`: 기본 0 은 시스템 최대 frame rate, 60fps 로 받으려면 1/60. https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration/minimumframeinterval
  - `queueDepth`: 기본값과 최솟값 모두 3, 8 을 넘기지 말 것. https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration/queuedepth
  - `captureResolution`: macOS 14.0. https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration/captureresolution
  - HDR 캡처용 `captureDynamicRange`: macOS 15.0. https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration/capturedynamicrange
- 커서: `SCStreamConfiguration` topic 목록에는 커서 모양이나 위치를 따로 넘겨주는 속성이 없고 `showsCursor`, `showMouseClicks`(15.0) 뿐이다. https://developer.apple.com/documentation/screencapturekit/scstreamconfiguration . 다른 앱의 현재 커서 모양을 얻던 `NSCursor.currentSystem` 은 macOS 27.0 에서 deprecated 되었고, 문서는 ScreenCaptureKit 의 `showsCursor` 를 쓰라고 안내한다. 커서 모양을 영상과 따로 보내는 방식(RustDesk Windows 방식)은 macOS 에서 대체 API 가 없다는 뜻이다. https://developer.apple.com/documentation/appkit/nscursor/currentsystem
- dirty rect: `SCStreamFrameInfo.dirtyRects`(12.3)는 다시 그려지거나 이동한 영역의 합집합. WWDC22 에서는 dirty rect 영역만 인코딩, 전송하고 받는 쪽에서 이전 frame 위에 덮어쓰는 방법을 소개했다. https://developer.apple.com/documentation/screencapturekit/scstreamframeinfo/dirtyrects , https://developer.apple.com/videos/play/wwdc2022/10155/
- 변화 없는 frame: frame status 가 `idle` 이면 새 IOSurface 가 없다. 즉 화면이 바뀌지 않으면 새 영상 frame 이 오지 않는다. 최대 해상도와 frame rate 는 디스플레이 native 값까지. https://developer.apple.com/videos/play/wwdc2022/10156/
- 지연과 frame 손실(WWDC22): `queueDepth` 를 늘리면 frame rate 에는 유리하지만 메모리와 지연이 늘 수 있다. frame 을 surface pool 에 돌려주는 시간이 `minimumFrameInterval x (queueDepth - 1)` 보다 길면 stall 이 생기고 frame 을 놓친다. 인코딩과 스트리밍에는 YUV 420(`420v`)을 권장한다. https://developer.apple.com/videos/play/wwdc2022/10155/
- `SCScreenshotManager`(단일 frame)와 `SCContentSharingPicker`(시스템 공유 선택 UI)는 macOS 14.0 부터. Apple 은 선택 UI 를 직접 만들지 말고 picker 를 쓰라고 권하지만, picker 는 host 쪽 사람이 직접 골라야 하므로 무인 원격 제어에는 맞지 않는다(추론). https://developer.apple.com/documentation/screencapturekit/scscreenshotmanager , https://developer.apple.com/documentation/screencapturekit/sccontentsharingpicker
- CGDisplayStream 과 CGWindowListCreateImage:
  - macOS 15 release notes: `CGDisplayStream`, `CGWindowListCreateImage` 같은 deprecated 캡처 API 를 쓰는 앱은 시스템 경고를 띄울 수 있으므로 ScreenCaptureKit 과 `SCContentSharingPicker` 로 옮기라고 한다. https://developer.apple.com/documentation/macos-release-notes/macos-15-release-notes
  - macOS 15 SDK 로 빌드하면 `CGDisplayStreamCreateWithDispatchQueue is unavailable: obsoleted in macOS 15.0 - Please use ScreenCaptureKit instead`, `CGWindowListCreateImage is unavailable: obsoleted in macOS 15.0` 컴파일 오류가 난다는 점은 SDK header 를 직접 열어 보지 못했고 제3자 빌드 로그로만 확인했다, 미확인(2차 출처). https://github.com/FreeRDP/FreeRDP/issues/10558 , https://trac.macports.org/ticket/71136
  - WWDC22 의 OBS 사례: `CGWindowListCreateImage` 7fps 에서 ScreenCaptureKit 60fps, CPU 사용량 최대 절반. https://developer.apple.com/videos/play/wwdc2022/10155/

### 10.2 Retina (points 와 pixels), 멀티 디스플레이

- macOS 는 화면 크기를 point 로 표현한다. backing scale factor 는 point 와 pixel 의 비율(1.0 또는 2.0)이고 view, window, screen 마다 따로 붙는다. https://developer.apple.com/library/archive/documentation/GraphicsAnimation/Conceptual/HighResolutionOSX/Explained/Explained.html
- `NSScreen.backingScaleFactor` 는 10.7 부터. https://developer.apple.com/documentation/appkit/nsscreen/backingscalefactor
- `SCDisplay.width` 는 point 단위(12.3). 출력 pixel 크기는 `SCShareableContentInfo.pointPixelScale`(14.0)로 구한다. https://developer.apple.com/documentation/screencapturekit/scdisplay/width , https://developer.apple.com/documentation/screencapturekit/scshareablecontentinfo/pointpixelscale
- 받은 frame 의 `scaleFactor` 가 표시할 디스플레이의 scale 과 다르면 그 비율로 크기를 맞춰야 한다. 안 그러면 2x 화면 내용이 1x 화면에서 4배 크기로 보인다. https://developer.apple.com/videos/play/wwdc2022/10155/
- display mode 는 `width`(10.6)와 `pixelWidth`(10.8)를 따로 준다. HiDPI 모드에서는 둘이 다르다. https://developer.apple.com/documentation/coregraphics/cgdisplaymode/pixelwidth . RustDesk 는 `pixelWidth > width` 로 HiDPI 모드를 판별한다. https://github.com/rustdesk/rustdesk/blob/master/src/platform/macos.mm
- 디스플레이 목록과 좌표: `CGGetActiveDisplayList`(10.0)의 첫 항목은 main display, hardware mirroring 에서는 primary 만 나온다. `CGDisplayBounds`(10.0)는 global display coordinate space 기준이고 기준점은 main display 의 왼쪽 위. main display 는 (0,0) 에 놓인 디스플레이다. 따라서 main 의 왼쪽이나 위에 놓인 디스플레이는 음수 좌표를 가진다(정의에서 따라 나오는 결론). https://developer.apple.com/documentation/coregraphics/cggetactivedisplaylist(_:_:_:) , https://developer.apple.com/documentation/coregraphics/cgdisplaybounds(_:) , https://developer.apple.com/documentation/coregraphics/cgmaindisplayid()
- 마우스 이벤트 좌표도 같은 global 좌표(point)를 쓴다. https://developer.apple.com/documentation/coregraphics/cgevent/init(mouseeventsource:mousetype:mousecursorposition:mousebutton:)
- 디스플레이 추가, 제거, 재구성 때는 `CGDisplayRegisterReconfigurationCallback`(10.3)이 재구성 전과 후에 한 번씩 불린다. https://developer.apple.com/documentation/coregraphics/cgdisplayregisterreconfigurationcallback(_:_:)

### 10.3 입력 주입

- `CGEvent` 로 마우스, 키보드 이벤트를 만들고 `post(tap:)` / `CGEventPost`(10.4)로 이벤트 흐름에 넣는다. 이 경로는 해당 위치의 event tap 을 모두 거친다. https://developer.apple.com/documentation/coregraphics/cgevent/post(tap:) , https://developer.apple.com/documentation/coregraphics/cgevent/init(keyboardeventsource:virtualkey:keydown:)
- 키 입력은 virtual key code 기준이고, 문자 하나를 만들 때도 modifier 를 포함한 모든 키 down/up 을 보내야 한다. `keyboardSetUnicodeString` 으로 Unicode 문자열을 직접 넣을 수 있지만 앱 framework 가 이를 무시하고 key code 로 다시 해석할 수 있다고 문서에 적혀 있다. https://developer.apple.com/documentation/coregraphics/cgevent/keyboardsetunicodestring(stringlength:unicodestring:)
- 휠: `kCGScrollEventUnitPixel` 은 대부분 앱이 smooth scroll 로 해석하고, 기본 배율은 1 line 에 약 10 pixel. wheel 3개를 받는 생성자는 10.13 부터. https://developer.apple.com/documentation/coregraphics/cgscrolleventunit , https://developer.apple.com/documentation/coregraphics/cgevent/init(scrollwheelevent2source:units:wheelcount:wheel1:wheel2:wheel3:)
- 권한 확인 함수: 이벤트 전송은 `CGPreflightPostEventAccess` / `CGRequestPostEventAccess`, 화면 캡처는 `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess`, 입력 감시는 `CGPreflightListenEventAccess`. 모두 10.15 부터이고 Apple 문서의 설명 본문은 비어 있다. https://developer.apple.com/documentation/coregraphics/cgpreflightposteventaccess() . `AXIsProcessTrustedWithOptions`(10.9)는 현재 프로세스가 신뢰받는 accessibility client 인지 알려 준다. https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions
- 사용자 승인: 앱이 accessibility 기능으로 Mac 을 제어하려 하면 경고가 뜨고, 사용자가 시스템 설정 > 개인정보 보호 및 보안 > 손쉬운 사용(Accessibility)에서 직접 허용해야 한다. https://support.apple.com/guide/mac-help/mh43185/mac
- viewer 가 macOS 일 때 단축키 가로채기: `CGEvent.tapCreate` 는 root 이거나 손쉬운 사용 허용이 필요하다고 (오래된) 문서에 적혀 있다. https://developer.apple.com/documentation/coregraphics/cgevent/tapcreate(tap:place:options:eventsofinterest:callback:userinfo:) . 별도로 Input Monitoring 권한("다른 앱을 쓰는 중에도 키보드, 마우스 감시")이 있다. https://support.apple.com/guide/mac-help/mchl4cedafb6/mac
- 키보드 배열과 한글: 배열별 문자 변환(`UCKeyTranslate`)과 입력 소스 전환(`TISSelectInputSource`)은 현재 developer.apple.com 에 문서가 없다. 옛 Text Input Source Services Reference 사본만 있어 미확인(2차 출처). https://leopard-adc.pepas.com/documentation/TextFonts/Reference/TextInputSourcesReference/TextInputSourcesReference.pdf
  - Windows viewer 의 `VK_HANGUL`, `VK_HANJA` 를 macOS host 에서 어떻게 재현할지(입력 소스 전환인지 특정 key code 인지)는 1차 출처로 확인하지 못했다. 확인 필요.
  - RustDesk 의 macOS 구현에서는 `Key::Hangul` 이 매핑되지 않아 `u16::MAX`(지원 안 함)로 떨어진다(`key_to_keycode`). https://github.com/rustdesk/rustdesk/blob/master/libs/enigo/src/macos/macos_impl.rs

### 10.4 권한 모델 (TCC, Transparency Consent and Control)

- 화면 녹화 권한은 시스템 설정 > 개인정보 보호 및 보안 > 화면 및 시스템 오디오 녹음 에서 앱마다 켜고 끈다. https://support.apple.com/guide/mac-help/mchld6aa7d23/mac
- MDM PPPC(Privacy Preferences Policy Control) payload `com.apple.TCC.configuration-profile-policy` 는 10.14 부터, user-approved MDM 이 필요하다. https://developer.apple.com/documentation/devicemanagement/privacypreferencespolicycontrol
- PPPC 서비스 키: https://developer.apple.com/documentation/devicemanagement/privacypreferencespolicycontrol/services-data.dictionary
  - `ScreenCapture`: profile 로 허용할 수 없고 거부만 가능(10.15+).
  - `ListenEvent`: 역시 거부만 가능(10.15+).
  - `PostEvent`: CGEvent 를 이벤트 흐름에 보내는 정책.
  - `Accessibility`: macOS 27.0 에서는 적용 시 앱마다 알림이 뜨고 사용자가 시스템 설정에서 바꿀 수 있다. 이 키는 macOS 27 에서 deprecated.
- PPPC `Authorization` 값 `AllowStandardUserToSetSystemService`(macOS 11+): 관리자가 아닌 표준 사용자도 해당 앱의 권한을 직접 켤 수 있게 한다. `ScreenCapture` 와 `ListenEvent` 에만 쓸 수 있다. 권한은 `CodeRequirement`(`codesign -display -r -` 결과)로 지정하므로 코드 서명과 묶인다. app bundle 안에 넣은 helper 도구는 감싸는 app bundle 의 권한을 물려받는다. https://developer.apple.com/documentation/devicemanagement/privacypreferencespolicycontrol/services-data.dictionary/identity
- macOS 27 부터 PPPC 키들은 deprecated 이고 declarative management `com.apple.configuration.app.settings` 의 `Privacy` 키를 쓰라고 안내한다. https://developer.apple.com/documentation/devicemanagement/appsettings
- 정리: 앱이 화면 녹화 권한을 스스로 얻을 방법은 없다. `CGRequestScreenCaptureAccess` 는 요청만 할 뿐이고 사람이 시스템 설정에서 허용해야 한다. MDM 도 허용 자체는 못 하고 표준 사용자에게 설정 권한을 넘기는 것까지만 된다(위 PPPC 문서에서 따라 나오는 결론).
- macOS 15 의 반복 확인 prompt:
  - 15.0: deprecated 캡처 API 를 쓰는 앱은 시스템 경고를 띄울 수 있다. https://developer.apple.com/documentation/macos-release-notes/macos-15-release-notes
  - 15.1: 이미 위험을 확인하고 받아들인 앱을 자주 쓰는 사용자에게는 경고를 덜 띄우도록 바뀌었다. https://developer.apple.com/documentation/macos-release-notes/macos-15_1-release-notes
  - 베타 기간에 주 1회에서 월 1회로 바뀌었고, 재시작, 로그아웃 후 prompt 는 없어졌으며, ScreenCaptureKit 만 써도 prompt 가 뜬다는 보고는 미확인(2차 출처). https://tidbits.com/2024/08/19/apple-reduces-excessive-sequoia-permission-requests-shifts-to-monthly/ , https://9to5mac.com/2024/08/14/macos-sequoia-screen-recording-prompt-monthly/
- 반복 prompt 를 피하는 공식 방법:
  - MDM Restrictions 의 `forceBypassScreenCaptureAlert`(15.1+)를 true 로 두면 화면 캡처 경고 표시를 건너뛴다. https://developer.apple.com/documentation/devicemanagement/restrictions
  - `com.apple.developer.persistent-content-capture` entitlement(14.4+)는 VNC 앱이 화면을 계속 보고 녹화할 수 있게 하는 권한이다. Apple 에 신청해 승인을 받아야 쓸 수 있다. https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.persistent-content-capture

### 10.5 로그인 화면, 잠금 화면, launchd

- TN2083 기준 역할 분담: https://developer.apple.com/library/archive/technotes/tn2083/_index.html
  - daemon 은 GUI 를 띄울 수 없고 window server 에 연결하면 안 된다. AppKit, ApplicationServices 처럼 CoreServices 위의 framework 는 daemon 에서 안전하지 않다. 따라서 LaunchDaemon 은 화면 캡처와 입력 주입을 직접 할 수 없다.
  - agent 는 `LimitLoadToSessionType` 을 `Aqua`(GUI 세션), `LoginWindow`(loginwindow 문맥), `Background`, `StandardIO` 로 지정할 수 있고 배열로 여러 개를 줄 수 있다. 지정하지 않으면 `Aqua`.
  - 로그인 전에 window server 에 연결하려면 pre-login launchd agent(`LoginWindow`)를 만든다. window server 가 종료되면 연결된 프로세스도 종료되고, 일반 로그아웃 뒤에는 살아남지 못한다.
- 로그인 창에서도 화면 녹화 TCC 권한이 똑같이 적용되는지는 1차 출처로 확인하지 못했다. 확인 필요.
- FileVault 가 켜진 Mac 은 재부팅 직후 잠금 해제 화면에서 원격 제어 소프트웨어가 동작하지 않고, `fdesetup authrestart` 로 한 번은 이 화면을 건너뛸 수 있다는 점은 미확인(2차 출처). https://www.macworld.com/article/2568036/how-to-manage-filevault-to-maintain-remote-access-to-your-mac.html , https://keith.github.io/xcode-man-pages/fdesetup.8.html

### 10.6 클립보드

- `NSPasteboard.changeCount` 는 pasteboard 소유권이 바뀔 때마다 1씩 늘어난다. 문서 topic 목록에 변경 알림 API 는 없으므로 changeCount 를 주기적으로 확인(polling)한다. https://developer.apple.com/documentation/appkit/nspasteboard/changecount , https://developer.apple.com/documentation/appkit/nspasteboard
- general pasteboard 는 10.12 부터 Universal Clipboard 에 자동으로 참여하고, 이를 제어하는 macOS API 는 없다. https://developer.apple.com/documentation/appkit/nspasteboard
- 자료형: `html`(10.6), `png`(10.6), `fileURL`(10.13) 등. https://developer.apple.com/documentation/appkit/nspasteboard/pasteboardtype/fileurl , https://developer.apple.com/documentation/appkit/nspasteboard/pasteboardtype/html
- 붙여넣기 개인정보 보호: macOS 15.4 에 `NSPasteboard.accessBehavior`(`alwaysAllow`, `alwaysDeny`, `ask`, `default`)가 추가됐다. 문서에 따르면 general pasteboard 는 기본적으로 "프로그램이 읽을 때 묻기" 이고, 첫 경고 뒤에는 앱이 시스템 설정에 나타나 사용자가 동작을 바꿀 수 있다. 내용을 읽지 않고 형식만 검사하는 `detectedPatterns(for:)` 도 15.4 부터. https://developer.apple.com/documentation/appkit/nspasteboard/accessbehavior-swift.enum , https://developer.apple.com/documentation/appkit/nspasteboard/detectedpatterns(for:)
  - macOS 27.2 release notes 에는 `EnablePasteboardPrivacyDeveloperPreview` user default 가 제거됐다고만 적혀 있어, 이 동작이 기본으로 켜졌는지는 확인하지 못했다. https://developer.apple.com/documentation/macos-release-notes/macos-27_2-release-notes
  - macOS 26 에서는 이 기능을 defaults 로 직접 켜야만 동작했다는 보고는 미확인(2차 출처). https://mjtsai.com/blog/2025/05/12/pasteboard-privacy-preview-in-macos-15-4/
  - host agent 가 클립보드를 동기화하려면 이 prompt 를 사용자가 한 번 허용해야 할 가능성이 있다(추론).
- 민감한 데이터 표시: `org.nspasteboard.ConcealedType`(기밀), `TransientType`(잠깐만 머무는 내용), `AutoGeneratedType`(앱이 자동으로 만든 내용). Apple 표준이 아니라 커뮤니티 관례다. http://nspasteboard.org/

### 10.7 인코딩 (VideoToolbox)

- VideoToolbox 는 하드웨어 encoder/decoder 에 직접 접근하는 framework 이고, `VTCompressionSession` 은 10.8 부터. https://developer.apple.com/documentation/videotoolbox/vtcompressionsession
- `kVTVideoEncoderSpecification_EnableLowLatencyRateControl`(macOS 11.3): GOP 를 끝없이 이어 가고(첫 IDR 뒤 모두 P frame), B frame 과 look-ahead 를 쓰지 않으며, High 계열 profile 과 temporal layer 구조를 쓴다. https://developer.apple.com/documentation/videotoolbox/kvtvideoencoderspecification_enablelowlatencyratecontrol
  - WWDC21: 이 모드는 H.264 만 지원, 720p 30fps 에서 지연을 최대 100ms 줄임. 네트워크 오류 복구용 LTR, `MaxAllowedFrameQP`, Constrained Baseline/High profile 도 함께 소개. https://developer.apple.com/videos/play/wwdc2021/10158/
- `kVTVideoEncoderSpecification_RequireHardwareAcceleratedVideoEncoder`(10.9)를 켜면 하드웨어를 쓸 수 없을 때(하드웨어 없음, 형식 미지원, 이미 바쁨) 세션 생성이 실패한다. https://developer.apple.com/documentation/videotoolbox/kvtvideoencoderspecification_requirehardwareacceleratedvideoencoder
- `AllowFrameReordering` 기본값 true, false 로 두면 B frame 이 없어진다. `RealTime`(10.9), `MaxFrameDelayCount`(10.8)도 지연 조절에 쓴다. https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_allowframereordering , https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_maxframedelaycount
- 4:4:4: `kVTCompressionPropertyKey_ProfileLevel` 상수 목록에 H.264 나 HEVC 의 4:4:4 profile 은 없다. HEVC 는 Main, Main10, Main42210(4:2:2 10bit, 12.3), Monochrome 까지. 따라서 글자 선명도용 4:4:4 하드웨어 인코딩은 VideoToolbox 에서 기대하기 어렵다(상수 목록에서 따라 나오는 결론). https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_profilelevel
- M3 계열부터 media engine 이 AV1 decode 를 지원한다. encode 는 지원하지 않는다. https://www.apple.com/newsroom/2023/10/apple-unveils-m3-m3-pro-and-m3-max-the-most-advanced-chips-for-a-personal-computer/

### 10.8 RustDesk macOS 구현 (master 브랜치, 조사일 확인)

- 캡처: ScreenCaptureKit 이 아니라 `CGDisplayStreamCreateWithDispatchQueue` 로 캡처하고 `FrameComplete` 인 frame 만 쓴다. 캡처 크기는 `CGDisplayPixelsWide x BackingScaleFactor` 로 계산하고 Retina 사용 설정에 따라 달라진다. https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/quartz/capturer.rs , https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/quartz/display.rs
- 입력: `CGEventPost(CGEventTapLocation::HID, ...)` 로 보내고, `TISCopyCurrentKeyboardLayoutInputSource` 와 `UCKeyTranslate` 로 배열을 처리한다. 휠은 line 단위 이벤트. https://github.com/rustdesk/rustdesk/blob/master/libs/enigo/src/macos/macos_impl.rs
- 권한 확인: 손쉬운 사용은 `AXIsProcessTrustedWithOptions`, 화면 녹화는 `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess`, 입력 감시는 `IOHIDCheckAccess` / `IOHIDRequestAccess(kIOHIDRequestTypeListenEvent)`. https://github.com/rustdesk/rustdesk/blob/master/src/platform/macos.rs , https://github.com/rustdesk/rustdesk/blob/master/src/platform/macos.mm
- launchd: LaunchAgent `agent.plist` 는 `LimitLoadToSessionType` 을 `LoginWindow` + `Aqua` 로 두고 `RustDesk --server` 를 실행한다. LaunchDaemon `daemon.plist` 는 `service` 를 `KeepAlive` 로 실행한다. `macos.rs` 에는 "UID 0 이 LoginWindow 세션 동안 /dev/console 을 소유한다", "prelogin 에서 마우스/키보드가 launchctl asuser 로 동작한다" 는 주석이 있고, `launchctl bootstrap gui/<uid>` 와 `load -S LoginWindow` 로 agent 를 올린다. https://github.com/rustdesk/rustdesk/blob/master/src/platform/privileges_scripts/agent.plist , https://github.com/rustdesk/rustdesk/blob/master/src/platform/privileges_scripts/daemon.plist , https://github.com/rustdesk/rustdesk/blob/master/src/platform/macos.rs
- 클립보드: `clipboard-master` 가 `changeCount` 를 polling 하고, 본체는 `CLIPBOARD_INTERVAL = 333`(ms)에 arboard 를 쓴다. https://github.com/rustdesk-org/clipboard-master/blob/master/src/master/mac.rs , https://github.com/rustdesk/rustdesk/blob/master/src/clipboard.rs
- 인코딩: hwcodec 경로에서 `videotoolbox` encoder 를 쓴다. https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/hwcodec.rs

## 11. 설계에 바로 영향을 주는 사실

아래 항목은 구조를 바꾸는 제약만 추렸다. 근거는 본문 해당 절에 모두 있고, 여기에도 대표 출처를 붙였다.

1. **UAC 동의 창, 로그인 화면, 잠금 화면을 보고 조작하려면 대상 세션 안에서 LocalSystem 으로 도는 프로세스가 필요하다.** secure desktop 은 LOCAL_SYSTEM 만 캡처할 수 있고, uiAccess 로는 system IL UI 에 닿지 못한다. 그래서 구조는 "session 0 의 LocalSystem 서비스 + 세션마다 띄우는 SYSTEM helper" 2단이 된다. RustDesk, TigerVNC 는 대상 세션 `winlogon.exe` 토큰으로 helper 를 띄운다. 서비스 없이 도는 모드(Quick Assist 포함)는 UAC 창을 host 앞 사람이 직접 눌러야 한다. (2.3, 2.5) https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput , https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview , https://learn.microsoft.com/en-us/windows/client-management/client-tools/quick-assist
2. **서비스는 session 0 에 갇혀 있어 화면, 입력, 클립보드를 직접 다루지 못한다.** 클립보드도 window station 마다 따로 있다. 캡처, 입력 주입, 클립보드 동기화는 모두 사용자 세션 helper 가 맡고, 서비스와 helper 사이 IPC 는 필수이며 ACL 을 걸어야 로컬 권한 상승 통로가 되지 않는다. (2.1, 7.1) https://learn.microsoft.com/en-us/windows/win32/services/interactive-services , https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclipboardsequencenumber
3. **desktop 이 바뀔 때마다(UAC, Ctrl+Alt+Del, 잠금) 캡처와 입력을 다시 붙여야 한다.** DDA 는 `DXGI_ERROR_ACCESS_LOST` 를 내고, 입력 스레드는 `SetThreadDesktop` 으로 옮겨야 하는데 창이나 hook 을 가진 스레드는 옮길 수 없다. 캡처와 입력은 창 없는 전용 스레드에서, 언제든 버리고 다시 만드는 상태 기계로 짜야 한다. 해상도 변경, 모니터 추가/제거, 전체화면 전환도 같은 경로를 탄다. (1.1, 2.4) https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe , https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddesktop
4. **기본 캡처 경로는 DDA 이고 WGC 는 보조 경로다.** WGC 는 SYSTEM 서비스 모드에서 동작하지 않았고(Sunshine 문서), UAC 가 보이지 않는다는 보고가 있다. WGC 테두리를 끄는 `IsBorderRequired` 는 build 20348 이상(Windows 11, Server 2022)에만 있어 Windows 10 client 에서는 끌 수 없고, 그 위에서도 사용자 동의 prompt 와 package capability 선언이 붙는다. (1.2) https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired , https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/docs/configuration.md#L2231-L2235
5. **DDA 는 output 이 붙은 GPU 에서만 된다.** hybrid 노트북의 외장 GPU 에서 돌리면 `DXGI_ERROR_UNSUPPORTED` 이고, 한 세션에서 동시에 쓸 수 있는 프로세스는 기본 4개다. adapter 별 D3D device 관리와 GDI fallback 이 필요하다. (1.1) https://learn.microsoft.com/en-us/troubleshoot/windows-client/shell-experience/error-when-dda-capable-app-is-against-gpu , https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput
6. **Ctrl+Alt+Del 은 viewer 에서 가로챌 수 없고, host 에서는 `SendSAS` 로만 보낸다.** `SendSAS` 는 서비스(또는 서명된 uiAccess 앱)여야 하고, `SoftwareSASGeneration` 정책이 설정되지 않은 기본 상태에서는 서비스의 SAS 가 막힌다. 설치 과정에서 정책을 설정하거나 RustDesk 처럼 호출 순간에만 바꿨다가 되돌려야 하며, viewer 에는 "특수 키 보내기" 메뉴가 필수다. (2.7, 4.2) https://learn.microsoft.com/en-us/windows/win32/api/sas/nf-sas-sendsas , https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-winlogon
7. **`SendInput` 이 UIPI 에 막혀도 반환값과 `GetLastError` 로 알 수 없다.** 사용자 토큰으로 띄운 helper 는 관리자 권한 창을 조작하지 못하고 실패도 모른다. injector 는 SYSTEM helper 안에서 돌려야 한다. (2.6, 3.1) https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
8. **host 프로세스는 Per-Monitor v2 DPI aware(Windows 10 1703+) 여야 하고, 모든 host 좌표는 physical pixel 기준 virtual desktop 좌표(음수 origin 포함)로 통일한다.** PMv2 가 아니면 좌표가 96 DPI 기준으로 가상화된다. `SendInput` 절대 좌표의 0..65535 반올림 규칙은 문서에 없고 오픈소스 세 곳이 다른 식을 쓰므로 되읽기 테스트가 필요하다. (3.2, 5.1, 6) https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows , https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput , https://learn.microsoft.com/en-us/windows/win32/gdi/the-virtual-screen
9. **키 입력 메시지에는 scancode(위치)와 문자(Unicode 또는 VK)를 둘 다 담고, host 가 mode 에 따라 고르는 구조로 가야 한다.** 한/영 키는 host 의 한국어 키보드 종류에 따라 오른쪽 Alt 가 `VK_RMENU` 도 되고 `VK_HANGUL` 도 되며, LANG1/LANG2 scancode 는 key release 때만 나온다. 휠은 칸 수가 아니라 원래 delta 를 그대로 보내야 고해상도 휠과 trackpad 스크롤이 유지된다. (3.2, 3.4, 3.5) https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input , https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
10. **viewer 의 `WH_KEYBOARD_LL` hook 은 제한 시간(1709 이후 최대 1000ms)을 넘기면 Windows 7 이후 알림 없이 제거된다.** 전용 thread 가 필수다. Ctrl+Alt+Del, Win+G 류는 hook 으로 막을 수 없고 Win+L 도 막을 수 없다는 보고(2차)가 있으므로, 메뉴와 대체 단축키(RDP 는 Ctrl+Alt+End)가 필요하다. (4.1, 4.2) https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc , https://learn.microsoft.com/en-us/windows/win32/termserv/imsrdpclientadvancedsettings-hotkeyctrlaltdel
11. **글자 선명도를 위한 YUV 4:4:4 는 경로가 좁다.** Windows 내장 H.264, HEVC decoder 는 4:2:0 만 되고 VideoToolbox 에도 4:4:4 profile 이 없다. 현실적인 경로는 소프트웨어 VP9/AV1(CPU 부하 큼) 또는 NVIDIA 끼리의 HEVC 4:4:4 다. 코덱과 chroma 를 연결마다 협상하는 구조가 처음부터 있어야 한다. (8.1, 8.4, 10.7) https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-decoder , https://learn.microsoft.com/en-us/windows/win32/medfound/h-265---hevc-video-decoder , https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_profilelevel
12. **소프트웨어 코덱 선택이 곧 라이선스 선택이다.** x264 는 GPL(상용 라이선스 별도), OpenH264 는 Cisco binary 를 사용자 기기에서 따로 내려받을 때만 H.264 로열티가 면제된다. VP8/VP9/AV1(libvpx, libaom, SVT-AV1, dav1d)은 BSD 계열에 로열티 없는 특허 허락이 붙는다. 참고 구현인 RustDesk 는 AGPL-3.0, Sunshine 은 GPL-3.0 이라 코드를 가져오면 그 의무가 따라온다. GeForce NVENC 는 시스템 전체 동시 세션이 12개다. (8.2, 8.3) https://www.openh264.org/BINARY_LICENSE.txt , https://www.videolan.org/developers/x264.html , https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-application-note/index.html
13. **원격 파일 붙여넣기는 `CF_HDROP` 으로 안 된다.** `OleSetClipboard` + `CFSTR_FILEDESCRIPTORW` / `CFSTR_FILECONTENTS` + `IStream` 으로 필요할 때 원격에서 끌어오는 delayed rendering 구조가 필요하고, 렌더링 중 막히면 붙여넣는 앱이 멈추므로 전송은 비동기 stream 이어야 한다. echo loop 방지 표식과 민감 데이터 제외 format(`ExcludeClipboardContentFromMonitorProcessing` 등)도 설계에 넣는다. (7.3, 7.4, 7.5) https://learn.microsoft.com/en-us/windows/win32/shell/clipboard , https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-operations , https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats
14. **배포는 서명해도 처음에는 SmartScreen 경고가 뜬다.** EV 인증서의 즉시 통과는 2024년에 없어졌고, Azure Artifact Signing 은 한국 조직이 대상 지역이 아니라 OV 인증서로 reputation 을 쌓아야 한다(수 주, 수백 건 설치). 활성 상태를 숨기거나 표준 제거를 막으면 Defender 의 unwanted software 기준에 걸리므로 "지금 원격 제어 중" 표시와 표준 제거 경로가 필요하다. 모니터 없는 PC 용 IddCx 가상 디스플레이를 넣으면 드라이버 서명(EV 인증서, attestation 은 테스트 용도) 부담이 더해진다. (9.1, 9.2, 9.4) https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options , https://learn.microsoft.com/en-us/defender-xdr/criteria , https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/driver-signing-offerings
15. **macOS host 는 사람의 1회 권한 허용이 반드시 필요하고, 프로세스 구조가 Windows 와 닮은 2단이다.** 화면 녹화 권한은 앱이 스스로 얻을 수 없고 MDM 도 허용은 못 한다(거부 또는 표준 사용자에게 설정 권한 위임만). macOS 15 이후 반복 경고를 공식으로 피하려면 MDM `forceBypassScreenCaptureAlert`(15.1+) 또는 Apple 승인이 필요한 `persistent-content-capture` entitlement(14.4+)가 필요하다. daemon 은 window server 에 붙을 수 없어 root LaunchDaemon + `LoginWindow`/`Aqua` LaunchAgent 로 나눈다. 캡처는 ScreenCaptureKit(12.3+)뿐이고 커서 모양 API 가 없으므로 커서를 영상에 합성해 보내는 방식도 프로토콜이 지원해야 한다. (10.1, 10.4, 10.5) https://developer.apple.com/documentation/devicemanagement/privacypreferencespolicycontrol/services-data.dictionary , https://developer.apple.com/documentation/devicemanagement/restrictions , https://developer.apple.com/library/archive/technotes/tn2083/_index.html , https://developer.apple.com/documentation/appkit/nscursor/currentsystem

## 12. RustDesk 구현 대조표 (항목 1~8)

RustDesk 는 AGPL-3.0 이다(https://github.com/rustdesk/rustdesk/blob/master/LICENCE). 동작을 이해하는 참고로만 쓰고 코드를 옮기지 않는다는 전제다. 링크는 대부분 commit `58ff78a8` 고정이고, 클립보드와 코덱은 master 기준이다.

| 항목 | RustDesk 가 하는 방식 | 소스 |
|---|---|---|
| 1. 화면 캡처 | DDA 기본, 실패하거나 프레임이 안 오면 GDI 로 전환. `IDXGIOutput6` 가 있으면 `DuplicateOutput1` 로 FP16 을 받아 tone-map. dirty/move rect 는 안 씀. WGC 안 씀. 커서는 영상에 넣지 않고 모양을 따로 보냄 | https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/dxgi/mod.rs , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/dxgi/hdr.rs , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/dxgi/gdi.rs , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/video_service.rs |
| 2. 권한 모델 | LocalSystem 서비스가 활성 세션(console 우선, 없으면 RDP)의 `winlogon.exe` 토큰으로 `--server` helper 를 SYSTEM 으로 띄우고, 세션 변화나 helper 종료를 감지하면 다시 띄움. 주입 직전마다 `try_change_desktop`. SAS 는 helper 가 IPC 로 서비스에 요청. portable 모드는 UAC 승인 후 SYSTEM 프로세스 + shared memory | https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.cc , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L832 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/service.rs , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/portable_service.rs |
| 3. 입력 주입 | enigo 로 `SendInput`(`MOUSEEVENTF_VIRTUALDESK`), rdev fork 로 scancode 와 Unicode 주입. Map / Translate / Legacy 세 mode. 한국어 layout 이면 `VK_RMENU` 를 `VK_HANGUL` 로 바꿈. 휠은 칸 단위, Windows trackpad 스크롤은 TODO. touch/pen 주입은 찾지 못함 | https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/input_service.rs , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/enigo/src/win/win_impl.rs , https://github.com/rustdesk-org/rdev/blob/a361d86a8b0245f3618a9efb375c149530a6b599/src/windows/simulate.rs |
| 4. viewer 단축키 | rdev fork `grab` 이 `WH_KEYBOARD_LL`, `WH_MOUSE_LL` 을 message loop thread 에 설치해 키를 먹음. CapsLock, NumLock 은 통과시키고 상태를 따로 맞춤. 포커스를 잃으면 눌린 키를 풀어 줌. "Insert Ctrl + Alt + Del", "Insert Lock" 메뉴 | https://github.com/rustdesk-org/rdev/blob/a361d86a8b0245f3618a9efb375c149530a6b599/src/windows/grab.rs , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/keyboard.rs , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/flutter/lib/common/widgets/toolbar.dart#L510-L540 |
| 5. 해상도, DPI | manifest 로 PerMonitorV2. 해상도 변경은 `CDS_UPDATEREGISTRY \| CDS_GLOBAL \| CDS_RESET` 으로 registry 에 저장하고, 연결이 0개가 되면 되돌림. 1초마다 display 목록 비교. headless 이면 Amyuni usbmmidd 또는 자체 IddDriver 로 가상 디스플레이 추가(19041+, 설치형만) | https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/res/manifest.xml , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/platform/windows.rs#L3066-L3095 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/display_service.rs , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/virtual_display_manager.rs |
| 6. 멀티 모니터 | display 마다 VideoService 하나. client 가 `SwitchDisplay` / `CaptureDisplays` 로 전환하거나 여러 개를 동시에 받음. DXGI 로 열거하고 실패하면 GDI | https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/video_service.rs#L296-L298 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/src/server/connection.rs#L4515-L4640 , https://github.com/rustdesk/rustdesk/blob/58ff78a823fe9ec5fe90f6017261c241656014c2/libs/scrap/src/common/dxgi.rs |
| 7. 클립보드 | clipboard-master(`AddClipboardFormatListener`)로 감지, arboard 로 읽고 씀. Text, Html, Rtf, 이미지(RGBA, PNG, SVG) 동기화. `dyn.com.rustdesk.owner` 표식으로 echo loop 방지. zstd 압축. 파일 복사-붙여넣기는 FreeRDP 에서 가져온 `wf_cliprdr.c`(OleSetClipboard + FILEDESCRIPTOR/FILECONTENTS). 클립보드 기록 제외 format 은 안 씀 | https://github.com/rustdesk/rustdesk/blob/master/src/clipboard.rs , https://github.com/rustdesk/rustdesk/blob/master/src/server/clipboard_service.rs , https://github.com/rustdesk/rustdesk/blob/master/libs/clipboard/src/windows/wf_cliprdr.c , https://github.com/rustdesk-org/clipboard-master/blob/master/src/master/win32.rs |
| 8. 인코딩 | 기본 VP9(libvpx), AV1(libaom, `AOM_CONTENT_SCREEN`), H.264/H.265 는 hwcodec(FFmpeg 의 nvenc, amf, qsv / d3d11va decode). 4:4:4 는 VP9, AV1 에서만, 모든 viewer 가 원할 때만. 실패하면 VP9 로 되돌아감. `video_qos.rs` 가 지연 150ms 기준으로 bitrate 를 먼저, FPS 를 나중에 조정 | https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/codec.rs , https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/vpxcodec.rs , https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/aom.rs , https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/common/hwcodec.rs , https://github.com/rustdesk/rustdesk/blob/master/src/server/video_qos.rs |

## 13. 확인하지 못한 것 (후속 검증 후보)

- unpackaged Win32 앱에서 WGC `RequestAccessAsync(Borderless)` 동의 prompt 가 어떻게 동작하는지. (1.2)
- WGC 가 SYSTEM 서비스 모드에서 실패하는 이유와 secure desktop 동작의 Microsoft 공식 설명. (1.2)
- `SoftwareSASGeneration` 값 0, 1, 2 의 의미를 적은 Microsoft 1차 문서(3 만 확인). (2.7)
- Windows 10 이후 LockApp 이 어느 desktop 에서 도는지. (2.8)
- Win+L 을 LL hook 으로 막을 수 없다는 공식 근거, 다른 프로세스 창의 IME mode 를 읽는 공식 방법. (3.5, 4.2)
- H.264 4:4:4 를 decode 할 수 있는 뷰어 쪽 decoder, AMD AMF 의 4:4:4 encode 지원, Intel 4:4:4 가 Windows 드라이버에서도 되는지. (8.2, 8.4)
- macOS 로그인 창 문맥에서 화면 녹화 TCC 권한이 어떻게 적용되는지, Windows 의 한/영, 한자 키를 macOS host 에서 재현하는 방법. (10.3, 10.5)
