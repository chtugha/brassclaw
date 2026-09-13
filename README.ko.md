<p align="center">
  <img src="brassclaw.png?v=2" alt="BrassClaw" width="200"/>
</p>

<h1 align="center">BrassClaw</h1>

<p align="center">
  <strong>완전히 당신의 하드웨어에서 실행되는 안전한 개인 AI 어시스턴트</strong>
</p>

<p align="center">
  <a href="https://github.com/chtugha/brassclaw/releases/latest"><img src="https://img.shields.io/github/v/release/chtugha/brassclaw?label=최신%20릴리즈" alt="Latest Release" /></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache%202.0-blue.svg" alt="License: MIT OR Apache-2.0" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.ru.md">Русский</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a>
</p>

<p align="center">
  <a href="#철학">철학</a> •
  <a href="#기능">기능</a> •
  <a href="#빠른-시작">빠른 시작</a> •
  <a href="#설정">설정</a> •
  <a href="#아키텍처">아키텍처</a>
</p>

---

## 철학

BrassClaw는 단순한 원칙 위에 만들어졌습니다: **AI 어시스턴트는 당신을 위해 일해야 합니다.**

- **100% 로컬 동작** — vLLM, Ollama, 또는 OpenAI 호환 서버로 동작. 클라우드 계정 불필요
- **데이터는 당신의 것** — 모든 상태는 로컬 내장 Postgres에 암호화되어 저장되며 당신의 통제를 벗어나지 않습니다
- **소비자 하드웨어 지원** — 7B 모델은 4GB VRAM으로 잘 작동
- **심층 방어** — 프로세스 샌드박스, 케이퍼빌리티 리스, 훅 프레임워크, 프롬프트 인젝션 방어
- **오픈 소스** — 완전히 감사 가능. 텔레메트리나 데이터 수집 없음
- **오케스트레이터 우선** — Monty 오케스트레이터가 실행 엔진으로 기능. LLM은 꼭 필요할 때만 사용

---

## 기능

### 오케스트레이터 우선 엔진

- **Monty (Python 오케스트레이터)** 가 레시피 단계를 순서대로 실행하고 Rust 도구를 직접 호출
- **Tier-0 레시피** — LLM 호출 없는 완전 결정론적 실행 경로
- **스킬** — 마크다운 파일이 API 사용 방법을 시스템에 가르침. Rust 컴파일 불필요
- **Sempai/Kohai 리뷰 루프** — 새 컴포넌트(레시피, 스킬, ToolSkill)를 자동 작성하고 검증 큐에 추가

### 보안 우선

- **프로세스 샌드박스** — 신뢰할 수 없는 도구 서브프로세스는 범위가 지정된 파일시스템과 엔드포인트 허용 목록으로 실행
- **훅 프레임워크** — 케이퍼빌리티 호출 및 프롬프트 변경에 대한 4개의 신뢰 티어(Builtin, Trusted, Installed, SelfAuthored)
- **케이퍼빌리티 리스** — 모든 도구 호출에 대한 세분화된 취소 가능 권한 부여
- **자격 증명 보호** — 시크릿은 도구에 노출되지 않음. 누출 감지와 함께 호스트 경계에서 주입
- **프롬프트 인젝션 방어** — 패턴 감지, 콘텐츠 삭제, 정책 시행
- **엔드포인트 허용 목록** — HTTP 요청은 명시적으로 승인된 호스트와 경로로만 전송

### 항상 사용 가능

- **다중 채널** — REPL, WebUI(React SPA, `/v2`), Slack, Telegram, HTTP 웹훅, API 서버
- **영구 메모리** — 내장 Postgres 기반 Reciprocal Rank Fusion을 사용한 하이브리드 전문 + 벡터 검색
- **루틴** — 백그라운드 자동화를 위한 cron 스케줄, 이벤트 트리거, 웹훅 핸들러
- **서브 에이전트** — 복잡한 작업을 위한 전문 자식 에이전트 스폰
- **MCP 프로토콜** — 모든 Model Context Protocol 서버에 연결

### 검증 파이프라인

모든 사용자 작성 및 에이전트 작성 컴포넌트는 2단계 검증 파이프라인을 거칩니다:

- **Q1** — 자동화된 오케스트레이티드 샌드박스 검사
- **Q2** — 인간 검토(운영자 전용, 자동화 불가)

---

## 빠른 시작

**최소 요구사항:** 약 8GB RAM, 최신 64비트 CPU, 약 4GB 여유 디스크 공간.

### 옵션 A: Linux 서버 — 원라인 설치 (권장)

GitHub 릴리즈에서 최신 빌드된 바이너리를 다운로드하고 systemd 서비스로 등록합니다:

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/install.sh | sudo bash
```

특정 버전 고정:

```bash
sudo bash install.sh -v 0.9.1
```

**제거:**

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/uninstall.sh | sudo bash
```

### 옵션 B: macOS — 수동 바이너리 설치

```bash
# Apple Silicon (M1/M2/M3):
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-arm64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn

# Intel Mac:
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-amd64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn
```

### 옵션 C: 소스에서 빌드

[Rust 1.94+](https://rustup.rs)가 필요합니다.

```bash
git clone https://github.com/chtugha/brassclaw.git
cd brassclaw
cargo build --release --bin brassclaw
```

바이너리는 `target/release/brassclaw`에 있습니다.

---

## 설정

### 설정 파일

기본 설정은 `~/.brassclaw/reborn/config.toml`에 있습니다. 첫 실행 시 예시가 생성됩니다.

```toml
[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
base_url = "http://localhost:8000/v1"

[identity]
tenant = "my-instance"
```

### 환경 변수

| 변수 | 설명 |
|------|------|
| `BRASSCLAW_REBORN_HOME` | 데이터 디렉토리 (기본값: `~/.brassclaw/reborn`) |
| `BRASSCLAW_RUNTIME_PROFILE` | 보안 정책 (기본값: `local_dev`). 유효값: `local_dev`, `local_safe`, `local_yolo`, `hosted_safe` |
| `BRASSCLAW_PG_URL` | 외부 Postgres URL. 생략 시 내장 Postgres 사용 |
| `BRASSCLAW_REBORN_WEBUI_TOKEN` | WebUI 인증 베어러 토큰 |
| `BRASSCLAW_REBORN_WEBUI_USER_ID` | 세션에 주입된 사용자 ID |

---

## 아키텍처

BrassClaw는 3레이어 모델을 사용합니다:

- **제품 레이어** — UX 소유. CLI, WebUI(`/v2`의 React SPA), Slack, Telegram
- **루프 레이어** — 에이전트 동작. Monty 오케스트레이터가 레시피 단계를 순서대로 실행하고 Rust 도구를 이름으로 호출
- **커널 레이어** — 권한 소유. LLM 프로바이더 추상화, 샌드박스 서브프로세스 실행, 자격 증명 주입, 보안 정책 시행

---

## 라이선스

다음 중 하나의 라이선스 하에 라이선스됩니다:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))
