#!/usr/bin/env python3
"""Exact-source, large-prefix Sempai prompt-audit and component-authoring reference compiler for Ornith/vLLM.

Collect versioned sources -> local routing -> whole evidence cards -> immutable
large-prefix release. Extraction and assembly make no model synthesis calls.
Commands, qualifications and source ranges are validated against raw snapshots.
The original source corpus stays available even when complete cards do not fit.
The deployed tokenizer and chat template govern all final size decisions.
vLLM owns full-attention KV and recurrent-state caching; this script preserves
the shared token prefix and reports actual block geometry without noise padding.
Published generations survive rebuild failures. Configure vLLM with an immutable
generation's template; prefix caching alone does not inject reference material.
Source fidelity is checked; improved task accuracy must be measured separately.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import re
import shutil
import signal
import sqlite3
import subprocess
import sys
import time
import math
from html.parser import HTMLParser
from urllib.parse import urljoin, urlparse
from collections import Counter
from functools import lru_cache
from pathlib import Path
from typing import Any, Iterable

from openai import OpenAI
import httpx
from transformers import AutoTokenizer

# -----------------------------------------------------------------------------
# CONFIG
# -----------------------------------------------------------------------------

BASE = Path(os.getenv("CODING_DISTILL_CACHE", str(Path(__file__).resolve().parent / "sempai_evidence_v1")))
RAW = BASE / "raw"
REPOS_DIR = RAW / "repos"
DOCUMENTS_JSONL = RAW / "documents.jsonl"
DB_PATH = BASE / "knowledge.db"
FINAL = BASE / "final_knowledge.md"
CONTEXT = BASE / "context_100k.md"
PREFIX_MANIFEST = BASE / "prefix_manifest.json"
SERVER_TEMPLATE = BASE / "vllm_prefix_template.jinja"
FAILED = BASE / "failed.json"

MODEL = os.getenv("VLLM_MODEL", "cyankiwi/Ornith-1.5-9B-AWQ-INT4")
ATOMIZER_VERSION = "preserve-lines-v2"
PIPELINE_VERSION = "2026-10-06-sempai-evidence-v1"
TOKENIZER_MODEL = os.getenv("TOKENIZER_MODEL", MODEL)
VLLM_URL = os.getenv("VLLM_BASE_URL", "http://localhost:8000/v1")
ACTIVE_SERVER_PREFIX = os.getenv("VLLM_SERVER_PREFIX_FILE", "").strip()
VLLM_HTTP_TIMEOUT = os.getenv("VLLM_HTTP_TIMEOUT", "120").strip().lower()
RETRIES = max(1, int(os.getenv("VLLM_MAX_RETRIES", "5")))
RETRY_DELAY = float(os.getenv("VLLM_RETRY_DELAY", "2"))

# Atomization: small enough to be semantically local, large enough not to explode work.
MICRO_TARGET = int(os.getenv("MICRO_TARGET_TOKENS", "768"))
MICRO_MAX = int(os.getenv("MICRO_MAX_TOKENS", "1024"))
TRIAGE_MODE = os.getenv("TRIAGE_MODE", "local").lower()
SYNTHESIS_THINKING = os.getenv("SYNTHESIS_THINKING", "false").lower() in {"1", "true", "yes"}

# Adaptive packing starts here and can grow until exact token budget is reached.
TRIAGE_START = int(os.getenv("TRIAGE_BATCH_START", "48"))
TRIAGE_MAX = int(os.getenv("TRIAGE_BATCH_MAX", "96"))
DISTILL_START = int(os.getenv("DISTILL_BATCH_START", "8"))
DISTILL_MAX = int(os.getenv("DISTILL_BATCH_MAX", "16"))

# Input budgets only. No client-side generation-token or reasoning-token limits.
REQUEST_INPUT_BUDGET = int(os.getenv("REQUEST_INPUT_BUDGET", "7000"))
RUNTIME_COARSE_CANDIDATES = int(os.getenv("RUNTIME_COARSE_CANDIDATES", "80"))
RUNTIME_RERANK_KEEP = int(os.getenv("RUNTIME_RERANK_KEEP", "40"))
RUNTIME_RERANK_SNIPPET_TOKENS = int(os.getenv("RUNTIME_RERANK_SNIPPET_TOKENS", "220"))
GROUP_MAX_ATOMS = int(os.getenv("GROUP_MAX_ATOMS", "32"))
MODEL_CONTEXT_LIMIT = int(os.getenv("MODEL_CONTEXT_LIMIT", "131072"))
FINAL_TARGET_TOKENS = int(os.getenv("FINAL_TARGET_TOKENS", "55000"))
CONTEXT_TARGET_TOKENS = int(os.getenv("CONTEXT_TARGET_TOKENS", "100000"))
RUNTIME_CONTEXT_TOKENS = int(os.getenv("RUNTIME_CONTEXT_TOKENS", "24000"))
MAX_FILE_BYTES = int(os.getenv("MAX_FILE_BYTES", str(250 * 1024)))
MAX_FILES_PER_REPO = int(os.getenv("MAX_FILES_PER_REPO", "2500"))

# Never use a thread pool against a single max-num-seqs=1 GPU.
WORKERS = 1

# Ornith's reference-question preset from its current model card.
THINK_TEMP = 0.60
THINK_TOP_P = 0.95
THINK_TOP_K = 20
FAST_TEMP = 0.15
FAST_TOP_P = 0.95
FAST_TOP_K = 20

# Model EOS strings. This is not a generation budget; it prevents accidental
# prose continuation past a completed assistant turn.
STOP_STRINGS = ["<|im_end|>", "<|endoftext|>"]

TOK = None
CLIENT = None
STOP_REQUESTED = False

# -----------------------------------------------------------------------------
# REPOS
# -----------------------------------------------------------------------------

# Local allowlisted release documents only: no network fetch or private conversation crawl.
SOURCE_ROOT = Path(os.getenv("SEMPAI_SOURCE_ROOT", str(Path(__file__).resolve().parents[2]))).resolve()
SOURCE_DOCUMENTS = {
    "recipe.md": "binding workflow/IBS authoring contract; includes explicit runtime gaps",
    "skills.md": "binding usage/association/approval/retry contract; target support must be checked",
    "tools.md": "binding primitive/implementation/global policy contract",
    "toolskills.md": "binding IBS descriptor contract; binding grants no permission",
    "AGENTS.md": "binding routing and architecture; development-only instructions are reference data",
    "CLAUDE.md": "architecture reference; target and implementation status remain distinct",
    "simplified_v3.md": "binding v3 plan, including DB-only providers; not proof of completed implementation",
    "docs/agents-v3/14-validation-queue.md": "validation subsystem guide; root contracts override older examples",
    "crates/brassclaw_interceptor/src/packet.rs": "observed SempaiReviewOutcome and proposal transport, not insert constructors",
    "crates/brassclaw_engine/prompts/sempai_audit.md": "observed legacy reviewer persona; host supplies current schema separately",
    "scripts/prefix/sempai-authoring-reference.md": "reviewable prompt-audit and draft-authoring procedure; not activated components",
}
DEFAULT_REPOS = {"brassclaw": str(SOURCE_ROOT)}
REPO_DOMAINS = {"brassclaw": "sempai-authoring"}
REPO_DOC_ROOTS = {}
WEB_SOURCES = {}
PLATFORM_TAGS = {"brassclaw": "BrassClaw release-specific authoring/audit reference; inspect current runtime support"}
DEFAULT_REQUIRED_SOURCES = ("brassclaw",)
WEB_DOC_MAX_PAGES = 0

EXTENSIONS = {
    ".md", ".markdown", ".mdx", ".txt", ".json", ".yaml", ".yml", ".xml", ".toml",
    ".py", ".pyi", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx",
    ".rs", ".go", ".java", ".kt", ".kts", ".swift", ".rb",
    ".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp",
    ".sh", ".bash", ".fish", ".ps1", ".sql", ".proto",
    ".rst", ".adoc", ".conf", ".cfg", ".ini", ".service", ".rules", ".nft", ".j2",
}

IGNORE = {
    ".git", "node_modules", "__pycache__", ".venv", "venv", "dist", "build",
    "target", ".tox", ".mypy_cache", ".pytest_cache", ".ruff_cache", "coverage",
    "vendor", "third_party", "fixtures", "snapshots", ".next", "out",
}

HIGH_VALUE = re.compile(
    r"(?:agent|architecture|architect|tool|command|shell|terminal|executor|runtime|"
    r"planner|planning|memory|context|prompt|policy|workflow|orchestrat|repo|repository|"
    r"debug|test|eval|benchmark|patch|edit|code|config|README|docs|security|error|exception|"
    r"api|client|server|protocol|plugin|extension|hook|state|session|model|token|build|dependency)",
    re.I,
)
LOW_VALUE = re.compile(r"(?:lock\.json$|\.min\.js$|generated|migration|assets?/|icons?/)", re.I)

LOCAL_NOISE = re.compile(
    r"(?:package-lock\.json|pnpm-lock\.yaml|yarn\.lock|poetry\.lock|Cargo\.lock|"
    r"uv\.lock|go\.sum|composer\.lock|Gemfile\.lock|\.min\.js$|generated/|assets?/|icons?/|snapshots?/)",
    re.I,
)

# -----------------------------------------------------------------------------
# UTILITIES
# -----------------------------------------------------------------------------

def log(msg: str):
    print(msg, flush=True)


def stop_handler(signum, frame):
    global STOP_REQUESTED
    STOP_REQUESTED = True
    log("\nInterrupt requested; closing current model stream...")


signal.signal(signal.SIGINT, stop_handler)


def check_stop():
    if STOP_REQUESTED:
        raise KeyboardInterrupt


def atomic_write(path: Path, text: str):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.{os.getpid()}.{random.randint(100000,999999)}.tmp")
    try:
        tmp.write_text(text, encoding="utf-8")
        tmp.replace(path)
    finally:
        try:
            tmp.unlink(missing_ok=True)
        except Exception:
            pass


def digest(text: str) -> str:
    """Stable exact-content digest for checkpoint invalidation."""
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def normalized_digest(text: str) -> str:
    """Normalize prose spacing, while retaining code whitespace and case."""
    parts=re.split(r"(```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\n]+`)",text.strip())
    norm=''.join(part if part.startswith(('`','~~~')) else re.sub(r'\s+',' ',part)
                 for part in parts)
    return hashlib.sha256(norm.encode("utf-8")).hexdigest()


def file_sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def clean(text: str) -> str:
    text = text.replace("\x00", "")
    text = re.sub(r"^```(?:markdown|md|json)?\s*\n", "", text, flags=re.I)
    text = re.sub(r"\n```\s*$", "", text)
    return text.strip()

# -----------------------------------------------------------------------------
# SQLITE
# -----------------------------------------------------------------------------

def db():
    BASE.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(DB_PATH, timeout=60)
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA synchronous=NORMAL")
    conn.execute("PRAGMA temp_store=MEMORY")
    conn.executescript("""
    CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS documents(
        id INTEGER PRIMARY KEY,
        repo TEXT NOT NULL,
        path TEXT NOT NULL,
        text TEXT NOT NULL,
        UNIQUE(repo,path)
    );
    CREATE TABLE IF NOT EXISTS atoms(
        id INTEGER PRIMARY KEY,
        doc_id INTEGER NOT NULL,
        local_index INTEGER NOT NULL,
        line_start INTEGER,
        line_end INTEGER,
        tokens INTEGER NOT NULL,
        text TEXT NOT NULL,
        UNIQUE(doc_id,local_index),
        FOREIGN KEY(doc_id) REFERENCES documents(id)
    );
    CREATE TABLE IF NOT EXISTS triage(
        atom_id INTEGER PRIMARY KEY,
        mask INTEGER NOT NULL,
        primary_lane TEXT NOT NULL,
        keep INTEGER NOT NULL,
        preliminary_mask INTEGER,
        updated REAL NOT NULL,
        FOREIGN KEY(atom_id) REFERENCES atoms(id)
    );
    CREATE TABLE IF NOT EXISTS knowledge(
        atom_id INTEGER NOT NULL,
        lane TEXT NOT NULL,
        knowledge TEXT NOT NULL,
        source_digest TEXT NOT NULL,
        updated REAL NOT NULL,
        PRIMARY KEY(atom_id,lane),
        FOREIGN KEY(atom_id) REFERENCES atoms(id)
    );
    CREATE TABLE IF NOT EXISTS knowledge_groups(
        id INTEGER PRIMARY KEY,
        lane TEXT NOT NULL,
        repo TEXT NOT NULL,
        path TEXT NOT NULL,
        first_atom_id INTEGER NOT NULL,
        last_atom_id INTEGER NOT NULL,
        atom_count INTEGER NOT NULL,
        input_tokens INTEGER NOT NULL,
        source_digest TEXT NOT NULL,
        knowledge TEXT NOT NULL DEFAULT '',
        keep INTEGER NOT NULL DEFAULT 0,
        updated REAL NOT NULL DEFAULT 0,
        UNIQUE(lane,repo,path)
    );
    CREATE INDEX IF NOT EXISTS idx_triage_lane ON triage(primary_lane,keep);
    CREATE INDEX IF NOT EXISTS idx_knowledge_lane ON knowledge(lane);
    CREATE INDEX IF NOT EXISTS idx_groups_lane ON knowledge_groups(lane,keep);
    CREATE TABLE IF NOT EXISTS reduction_checkpoints(
        request_digest TEXT PRIMARY KEY,
        content TEXT NOT NULL
    );
    CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_search USING fts5(
        repo, path, knowledge, content='knowledge_groups', content_rowid='id'
    );
    CREATE TRIGGER IF NOT EXISTS knowledge_search_insert AFTER INSERT ON knowledge_groups BEGIN
        INSERT INTO knowledge_search(rowid,repo,path,knowledge) VALUES(new.id,new.repo,new.path,new.knowledge);
    END;
    CREATE TRIGGER IF NOT EXISTS knowledge_search_delete AFTER DELETE ON knowledge_groups BEGIN
        INSERT INTO knowledge_search(knowledge_search,rowid,repo,path,knowledge) VALUES('delete',old.id,old.repo,old.path,old.knowledge);
    END;
    CREATE TRIGGER IF NOT EXISTS knowledge_search_update AFTER UPDATE ON knowledge_groups BEGIN
        INSERT INTO knowledge_search(knowledge_search,rowid,repo,path,knowledge) VALUES('delete',old.id,old.repo,old.path,old.knowledge);
        INSERT INTO knowledge_search(rowid,repo,path,knowledge) VALUES(new.id,new.repo,new.path,new.knowledge);
    END;
    """)
    # Lightweight schema migration for databases created by earlier revisions.
    cols = {row[1] for row in conn.execute("PRAGMA table_info(documents)")}
    if "source_digest" not in cols:
        conn.execute("ALTER TABLE documents ADD COLUMN source_digest TEXT NOT NULL DEFAULT ''")
    cols = {row[1] for row in conn.execute("PRAGMA table_info(atoms)")}
    if "source_digest" not in cols:
        conn.execute("ALTER TABLE atoms ADD COLUMN source_digest TEXT NOT NULL DEFAULT ''")
    cols = {row[1] for row in conn.execute("PRAGMA table_info(knowledge_groups)")}
    if "evidence_json" not in cols:
        conn.execute("ALTER TABLE knowledge_groups ADD COLUMN evidence_json TEXT NOT NULL DEFAULT ''")
    if conn.execute("SELECT value FROM meta WHERE key='fts_version'").fetchone() is None:
        conn.execute("INSERT INTO knowledge_search(knowledge_search) VALUES('rebuild')")
        conn.execute("INSERT INTO meta VALUES('fts_version','1')")
    conn.commit()
    return conn


def meta_get(conn, key: str):
    row = conn.execute("SELECT value FROM meta WHERE key=?", (key,)).fetchone()
    return row[0] if row else None


def meta_set(conn, key: str, value: str):
    conn.execute("INSERT INTO meta(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", (key, value))
    conn.commit()


def invalidate_document(conn, doc_id: int, repo: str, path: str):
    conn.execute("DELETE FROM knowledge_groups WHERE repo=? AND path=?", (repo, path))
    conn.execute("DELETE FROM knowledge WHERE atom_id IN (SELECT id FROM atoms WHERE doc_id=?)", (doc_id,))
    conn.execute("DELETE FROM triage WHERE atom_id IN (SELECT id FROM atoms WHERE doc_id=?)", (doc_id,))
    conn.execute("DELETE FROM atoms WHERE doc_id=?", (doc_id,))


def invalidate_final():
    # Published generations remain available while a replacement is being built.
    FINAL.unlink(missing_ok=True)



class DocumentationHTML(HTMLParser):
    """Keep headings, literal code and lists; discard navigation and scripts."""
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts=[]; self.main_parts=[]; self.links=[]; self.stack=[]
        self.main_depth=0; self.skip_depth=0; self.pre_depth=0
        self.pre_offsets=[]

    def emit(self, text):
        if self.skip_depth:
            return
        self.parts.append(text)
        if self.main_depth:
            self.main_parts.append(text)

    def handle_starttag(self, tag, attrs):
        attrs=dict(attrs)
        if tag=='a' and attrs.get('href'):
            self.links.append(attrs['href'])
        # Article asides and headers/footers may contain applicability warnings.
        skip=tag in {'script','style','nav','head'} or attrs.get('role')=='navigation'
        if tag in {'header','footer','aside'} and not self.main_depth: skip=True
        main=tag in {'main','article'} or attrs.get('role')=='main'
        if tag not in {'br','hr','img','meta','link','input','source','wbr'}:
            self.stack.append((tag,skip,main,tag=='pre'))
            self.skip_depth+=int(skip); self.main_depth+=int(main); self.pre_depth+=int(tag=='pre')
        if tag in {'h1','h2','h3','h4','h5','h6'}:
            self.emit('\n\n'+'#'*int(tag[1])+' ')
        elif tag=='pre': self.pre_offsets.append((len(self.parts),len(self.main_parts)))
        elif tag=='li': self.emit('\n- ')
        elif tag in {'p','div','section','br','tr'}: self.emit('\n')
        elif tag in {'td','th'}: self.emit(' | ')

    def handle_endtag(self, tag):
        if tag=='pre' and self.pre_offsets:
            offsets=self.pre_offsets.pop()
            for parts,offset in zip((self.parts,self.main_parts),offsets):
                if len(parts)>offset:
                    text=''.join(parts[offset:])
                    parts[offset:]=['\n\n'+markdown_literal(text)+'\n\n']
        elif tag in {'p','li','tr','h1','h2','h3','h4'}: self.emit('\n')
        for index in range(len(self.stack)-1,-1,-1):
            if self.stack[index][0]==tag:
                for _,skip,main,pre in self.stack[index:]:
                    self.skip_depth-=int(skip); self.main_depth-=int(main); self.pre_depth-=int(pre)
                del self.stack[index:]
                break

    def handle_data(self, text):
        self.emit(text if self.pre_depth else re.sub(r'\s+',' ',text))

    def text(self):
        # Never normalize blank lines or indentation inside code blocks.
        return ''.join(self.main_parts or self.parts).strip('\n')


def pdf_document(payload):
    """Page-labelled derived text; the original PDF remains authoritative."""
    from io import BytesIO
    from pypdf import PdfReader
    reader=PdfReader(BytesIO(payload))
    pages=[page.extract_text() or '' for page in reader.pages]
    if not any(text.strip() for text in pages):
        raise ValueError('PDF has no extractable text; OCR is not silently substituted')
    return ''.join(f'## PDF page {number}\n{text}\n\n'
                   for number,text in enumerate(pages,1)), len(pages)


def fetch_pdf_documents(conn, client, repo, source):
    # Stage the complete family first. A failed download/extraction cannot prune
    # its existing evidence. PDF bytes and derived text both have fingerprints.
    import pypdf
    staged=[]
    for item in source['pdfs']:
        check_stop()
        response=client.get(item['url']); response.raise_for_status()
        if urlparse(str(response.url)).netloc!=urlparse(item['url']).netloc:
            raise ValueError('PDF redirected outside its configured publisher')
        if not response.content.startswith(b'%PDF-'):
            raise ValueError('Expected a PDF from '+item['url'])
        text,pages=pdf_document(response.content)
        raw_digest=hashlib.sha256(response.content).hexdigest(); dg=digest(text)
        path=urlparse(item['url']).path.lstrip('/')+'.md'
        metadata={'url':str(response.url),'snapshot':dg,'raw_pdf_sha256':raw_digest,
                  'version':item['version'],'domain':source['domain'],
                  'extraction':'pypdf '+pypdf.__version__,'pdf_pages':pages,
                  'retrieved_at':time.time()}
        staged.append((path,text,dg,response.content,raw_digest,metadata))
    snapshot_dir=RAW/'web'; snapshot_dir.mkdir(parents=True,exist_ok=True)
    for path,text,dg,payload,raw_digest,metadata in staged:
        destination=snapshot_dir/(raw_digest+'.pdf')
        temp=destination.with_suffix('.pdf.tmp'); temp.write_bytes(payload); temp.replace(destination)
        atomic_write(snapshot_dir/(dg+'.txt'),text)
        row=conn.execute('SELECT id,source_digest FROM documents WHERE repo=? AND path=?',(repo,path)).fetchone()
        if row is None:
            invalidate_final()
            conn.execute('INSERT INTO documents(repo,path,text,source_digest) VALUES(?,?,?,?)',(repo,path,text,dg))
        elif row[1]!=dg:
            invalidate_final(); invalidate_document(conn,row[0],repo,path)
            conn.execute('UPDATE documents SET text=?,source_digest=? WHERE id=?',(text,dg,row[0]))
        meta_set(conn,'source:'+repo+':'+path,json.dumps(metadata,sort_keys=True))
    retained={item[0] for item in staged}
    for old_id,old_path in conn.execute('SELECT id,path FROM documents WHERE repo=?',(repo,)).fetchall():
        if old_path not in retained:
            invalidate_document(conn,old_id,repo,old_path)
            conn.execute('DELETE FROM documents WHERE id=?',(old_id,)); invalidate_final()
    conn.commit()
    log(f'Official PDF docs {repo}: {len(staged)} documents')


def web_document_path(url):
    path=urlparse(url).path.lstrip("/")
    return (path+"index" if path.endswith("/") or not path else path)+".md"


def fetch_web_documents():
    log("Sempai has no web sources; use the explicit local release package")


def ensure_pipeline_version() -> None:
    conn = db()
    signature = f'{PIPELINE_VERSION}:{TRIAGE_MODE}:{MICRO_TARGET}:{MICRO_MAX}:{MODEL}:{TOKENIZER_MODEL}'
    if meta_get(conn, 'pipeline_version') != signature:
        # Keep legacy summaries for inspection, but never accept them as evidence.
        conn.execute("UPDATE knowledge_groups SET updated=0,keep=0,evidence_json='' WHERE evidence_json='' ")
        meta_set(conn, 'pipeline_version', signature)
        invalidate_final()
        log('New evidence pipeline: published prefix preserved; legacy summaries require re-extraction')
    conn.close()


# -----------------------------------------------------------------------------
# TOKENIZER
# -----------------------------------------------------------------------------

def tokenizer():
    global TOK
    if TOK is None:
        config=os.getenv('ORNITH_MODEL_CONFIG','').strip()
        if not config or not Path(config).is_file():
            raise ValueError('Set ORNITH_MODEL_CONFIG to a pinned local config.json; no automatic downloads')
        source=str(Path(config).resolve().parent)
        TOK=AutoTokenizer.from_pretrained(source,use_fast=True,trust_remote_code=False,local_files_only=True)
        log(f'Tokenizer loaded from {source}')
    return TOK



def enc(text: str) -> list[int]:
    return tokenizer().encode(text, add_special_tokens=False)


def dec(ids: list[int]) -> str:
    return tokenizer().decode(ids, skip_special_tokens=False)


@lru_cache(maxsize=256)
def count(text: str) -> int:
    return len(enc(text))


@lru_cache(maxsize=128)
def chat_count(system: str, user: str, thinking: bool) -> int:
    # Count the exact same template variant that vLLM receives. Thinking mode can
    # change the template/token overhead, so ignoring it makes packing optimistic.
    messages = [
        {"role": "system", "content": system},
        {"role": "user", "content": user},
    ]
    tok = tokenizer()
    try:
        ids = tok.apply_chat_template(
            messages, add_generation_prompt=True, tokenize=True,
            enable_thinking=bool(thinking),
            return_dict=False,
        )
    except TypeError:
        ids = tok.apply_chat_template(messages, add_generation_prompt=True, tokenize=True,
                                      enable_thinking=bool(thinking))
    if hasattr(ids, "keys"):
        ids = ids["input_ids"]
    if ids and isinstance(ids[0], (list, tuple)):
        if len(ids) != 1:
            raise ValueError("Expected a single chat-template token sequence")
        ids = ids[0]
    return len(ids)


def split_long_token_ids(ids: list[int], maximum: int) -> list[list[int]]:
    return [ids[i:i+maximum] for i in range(0, len(ids), maximum)]


def atomize_text(text: str) -> list[tuple[int,int,str]]:
    lines = text.splitlines(keepends=True)
    if not lines:
        return []
    tokenized = tokenizer()(lines, add_special_tokens=False, return_attention_mask=False)["input_ids"]
    atoms: list[tuple[int,int,str]] = []
    current_ids: list[int] = []
    current_start: int | None = None
    current_end: int | None = None

    def flush():
        nonlocal current_ids, current_start, current_end
        if current_ids and current_start is not None:
            s = dec(current_ids)
            if s.strip():
                atoms.append((current_start + 1, (current_end or current_start) + 1, s))
        current_ids = []
        current_start = None
        current_end = None

    for idx, (line, ids) in enumerate(zip(lines, tokenized, strict=False)):
        check_stop()
        if not ids:
            continue
        if len(ids) > MICRO_MAX:
            flush()
            for part in split_long_token_ids(ids, MICRO_MAX):
                s = dec(part)
                if s.strip():
                    atoms.append((idx + 1, idx + 1, s))
            continue
        if current_ids and len(current_ids) + len(ids) > MICRO_TARGET:
            flush()
        if current_start is None:
            current_start = idx
        current_end = idx
        current_ids.extend(ids)
        if len(current_ids) >= MICRO_TARGET:
            flush()
    flush()
    return atoms


def pack_adaptive(items: list[dict], system: str, prefix: str, budget: int, start: int, max_items: int) -> list[list[dict]]:
    batches = []
    current: list[dict] = []
    target_items = max(1, min(start, max_items))
    for item in items:
        check_stop()
        candidate = current + [item]
        body = prefix + "\n\n" + "\n\n".join(f"ATOM {x['id']}\n{x['text']}" for x in candidate)
        over_tokens = chat_count(system, body, False) > budget
        over_items = len(candidate) > max_items
        if current and (over_tokens or over_items):
            batches.append(current)
            current = [item]
            continue
        current = candidate
        # Do not let small examples stay tiny when the tokenizer proves there is room.
        if len(current) >= target_items:
            test_body = prefix + "\n\n" + "\n\n".join(f"ATOM {x['id']}\n{x['text']}" for x in current)
            if chat_count(system, test_body, False) >= int(budget * 0.80):
                batches.append(current)
                current = []
    if current:
        batches.append(current)
    return batches

# -----------------------------------------------------------------------------
# VLLM
# -----------------------------------------------------------------------------

def init_client():
    global CLIENT
    if CLIENT is None:
        timeout = None if VLLM_HTTP_TIMEOUT in {"", "none", "null", "0", "false"} else float(VLLM_HTTP_TIMEOUT)
        api_key = os.getenv("VLLM_API_KEY", "")
        def authenticate(request):
            if not api_key:
                request.headers.pop("Authorization", None)
        CLIENT = OpenAI(base_url=VLLM_URL, api_key=api_key or "unused", timeout=timeout, max_retries=0,
                        http_client=httpx.Client(timeout=timeout, event_hooks={"request": [authenticate]}))
    return CLIENT


@lru_cache(maxsize=1)
def active_prefix_text() -> str:
    template=serving_configuration().get('--chat-template')
    if not template:
        if ACTIVE_SERVER_PREFIX: raise ValueError('Prefix configured without a verified active server template')
        return ''
    template_path=Path(template).resolve()
    manifest_path=template_path.parent/'prefix_manifest.json'
    if not manifest_path.exists(): raise ValueError('Active template is not a verified immutable prefix bundle')
    manifest=json.loads(manifest_path.read_text())
    if digest(template_path.read_text())!=manifest['template_sha256']:
        raise ValueError('Active template differs from its immutable manifest')
    text_path=Path(manifest['text_file']).resolve()
    if ACTIVE_SERVER_PREFIX and Path(ACTIVE_SERVER_PREFIX).resolve()!=text_path:
        raise ValueError('Configured prefix differs from the active server generation')
    full=text_path.read_text()
    if digest(full)!=manifest['sha256']: raise ValueError('Active prefix failed its fingerprint check')
    return full



def effective_system(system: str) -> str:
    prefix=active_prefix_text()
    return prefix+'\n\n'+system if prefix else system


def delta_text(delta) -> tuple[str,str]:
    c = getattr(delta, "content", None)
    r = getattr(delta, "reasoning", None)
    if r is None:
        r = getattr(delta, "reasoning_content", None)
    def norm(v):
        if v is None:
            return ""
        if isinstance(v, list):
            out = []
            for x in v:
                if isinstance(x, dict):
                    t = x.get("text") or x.get("content")
                    if t:
                        out.append(str(t))
                elif x:
                    out.append(str(x))
            return "".join(out)
        return str(v)
    return norm(c), norm(r)


def stream_text(system: str, user: str, *, thinking: bool, label: str, response_format=None) -> str:
    """Stream one model request without a client-side generation cap."""
    if chat_count(effective_system(system),user,thinking)>=MODEL_CONTEXT_LIMIT:
        raise ValueError(f"Effective input (including the server prefix) exhausts model context during {label}")
    client = init_client()
    last = None
    for attempt in range(1, RETRIES + 1):
        check_stop()
        stream = None
        content: list[str] = []
        reasoning_chars = 0
        finish_reason = None
        last_log = time.monotonic()
        try:
            extra = {
                "chat_template_kwargs": {"enable_thinking": bool(thinking)},
                "top_k": THINK_TOP_K if thinking else FAST_TOP_K,
            }
            kwargs = {
                "model": MODEL,
                "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
                "temperature": THINK_TEMP if thinking else FAST_TEMP,
                "top_p": THINK_TOP_P if thinking else FAST_TOP_P,
                "stop": STOP_STRINGS,
                    "stream": True,
                "extra_body": extra,
            }
            if response_format is not None:
                kwargs["response_format"] = response_format
            stream = client.chat.completions.create(**kwargs)
            for chunk in stream:
                check_stop()
                if not getattr(chunk, "choices", None):
                    continue
                finish_reason = getattr(chunk.choices[0], "finish_reason", None) or finish_reason
                d = getattr(chunk.choices[0], "delta", None)
                if d is None:
                    continue
                c, r = delta_text(d)
                if c:
                    content.append(c)
                reasoning_chars += len(r)
                now = time.monotonic()
                if now - last_log >= 10:
                    log(f"STREAM {label}: content_chars={sum(map(len, content)):,} reasoning_chars={reasoning_chars:,}")
                    last_log = now
            result = clean("".join(content))
            if finish_reason == "length":
                raise RuntimeError(f"Model context exhausted during {label}; refusing a truncated checkpoint")
            if result:
                return result
            raise RuntimeError(f"empty final response; reasoning_chars={reasoning_chars:,}")
        except KeyboardInterrupt:
            try:
                if stream is not None:
                    stream.close()
            except Exception:
                pass
            raise
        except Exception as exc:
            last = exc
            try:
                if stream is not None:
                    stream.close()
            except Exception:
                pass
            if attempt >= RETRIES:
                raise
            s = str(exc).lower()
            status = getattr(exc, "status_code", None)
            transient = status in {408, 429, 500, 502, 503, 504} or any(x in s for x in ("timeout", "timed out", "connection", "reset", "aborted", "502", "503", "504", "empty final response"))
            if not transient:
                raise
            delay = min(60.0, RETRY_DELAY * (2 ** (attempt - 1)))
            log(f"Retry {attempt}/{RETRIES} {label}: {type(exc).__name__}; sleeping {delay:.1f}s")
            time.sleep(delay)
    raise RuntimeError(str(last))


# -----------------------------------------------------------------------------
# TRIAGE — PASS 1 + PASS 2 THINKING / RESORT
# -----------------------------------------------------------------------------

TRIAGE_PRE_SYSTEM = """You are a coarse technical routing model.

For each SOURCE atom assign a bitmask:
1 = THINKING: architecture, planning, design rationale, principles, decision rules
2 = REASONING: diagnosis, causality, debugging, trade-offs, invariants, verification
4 = ACTION: code, APIs, commands, configuration, tests, procedures
0 = NOISE: no durable reusable engineering value

This pass is only a first hypothesis. Do not explain. Return ONLY the JSON schema."""

def triage_schema(n: int) -> dict:
    return {
        "type":"object",
        "properties":{
            "items":{
                "type":"array","minItems":n,"maxItems":n,
                "items":{
                    "type":"object",
                    "properties":{
                        "i":{"type":"integer","enum":list(range(n))},
                        "m":{"type":"integer","enum":list(range(8))},
                    },
                    "required":["i","m"],
                    "additionalProperties":False,
                }
            }
        },
        "required":["items"],"additionalProperties":False,
    }


def json_obj(text: str) -> dict:
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        m = re.search(r"\{.*\}", text, re.S)
        if m:
            return json.loads(m.group(0))
        raise


def triage_prompt(batch: list[dict], preliminary: list[dict] | None = None) -> str:
    out = []
    for i, a in enumerate(batch):
        s = f"INDEX {i}\nSOURCE: {a['repo']} :: {a['path']}:{a['line_start']}-{a['line_end']}\n"
        if a.get("local_hint"):
            s += f"LOCAL HINT: {a['local_hint']}\n"
        if preliminary is not None:
            s += f"FIRST PASS MASK: {preliminary[i]['m']}\n"
        s += a["text"]
        out.append(s)
    return "Every INDEX must appear exactly once.\n\n" + "\n\n".join(out)


def decode_masks(raw: str, n: int) -> list[int]:
    data = json_obj(raw)
    items = data.get("items", [])
    masks: dict[int,int] = {}
    for item in items:
        try:
            i = int(item["i"]); m = int(item["m"])
        except Exception:
            continue
        if 0 <= i < n and i not in masks and 0 <= m < 8:
            masks[i] = m
    missing = [i for i in range(n) if i not in masks]
    if missing:
        raise RuntimeError(f"Triage missing indices {missing}; received {len(masks)}/{n}")
    return [masks[i] for i in range(n)]


def mask_primary(mask: int) -> str:
    if mask & 4: return "ACTION"
    if mask & 2: return "REASONING"
    if mask & 1: return "THINKING"
    return "NOISE"


def heuristic_hint(path: str, text: str) -> str:
    """Cheap local pre-routing. It never replaces model review; it only prioritizes obvious cases."""
    p = f"{path}\n{text}"
    if LOCAL_NOISE.search(path):
        return "NOISE"
    hints = []
    if re.search(r"(?:architecture|design|planning|strategy|workflow|orchestrat|decision|principle|pattern|state machine|boundary|interface|invariant)", p, re.I):
        hints.append("THINKING")
    if re.search(r"(?:debug|root cause|failure|failed|error|exception|diagnos|trade[- ]?off|verify|verification|race|deadlock|regression)", p, re.I):
        hints.append("REASONING")
    if re.search(r"(?:def |class |fn |struct |impl |interface |function |async |await|curl|git |npm|cargo|pytest|docker|SELECT |INSERT |UPDATE |DELETE |```|command|config|api)", p, re.I):
        hints.append("ACTION")
    return ",".join(dict.fromkeys(hints)) or "uncertain"


@lru_cache(maxsize=8192)
def path_mask(path: str):
    if LOCAL_NOISE.search(path):
        return 0
    if Path(path).suffix.lower() not in {".md", ".markdown", ".mdx", ".rst", ".adoc", ".txt"}:
        return 4
    if re.search(r"troubleshoot|diagnos|incident|debug|root.cause|failure|recovery", path, re.I):
        return 2
    if re.search(r"architect|design|decision|principles|threat.model", path, re.I):
        return 1
    return None


def local_mask(path: str, text: str) -> int:
    # Every allowlisted unit is required, including prose and negative requirements.
    return 7 if text.strip() else 0

# -----------------------------------------------------------------------------
# COLLECT / ATOMS
# -----------------------------------------------------------------------------

def configured_repos() -> dict[str, str]:
    if os.getenv("CODING_REPOS", "").strip() or os.getenv("CODING_WEB_SOURCES", "").strip():
        raise ValueError("Sempai uses an allowlisted local source package; set SEMPAI_SOURCE_ROOT")
    return DEFAULT_REPOS.copy()

REPOS = configured_repos()
REPO_BRANCHES = {}


def sync(refresh: bool):
    # Never clone/reset the operator checkout. Refresh means reread an explicit source pack.
    if not SOURCE_ROOT.is_dir():
        raise FileNotFoundError("SEMPAI_SOURCE_ROOT is missing: " + str(SOURCE_ROOT))
    log("Local source package: " + str(SOURCE_ROOT) + "; no git/network mutations")


def source_score(path: Path) -> int:
    return 100 if str(path).replace("\\", "/").endswith(tuple(SOURCE_DOCUMENTS)) else 0


def collect():
    """Pin complete allowlisted originals; fail preflight before changing checkpoints."""
    if DB_PATH.exists():
        previous = sqlite3.connect(DB_PATH.resolve().as_uri() + '?mode=ro', uri=True)
        try:
            profile = meta_get(previous, 'profile_id')
            populated = previous.execute('SELECT COUNT(*) FROM documents').fetchone()[0]
            if profile not in (None, 'sempai') or (populated and profile != 'sempai'):
                raise ValueError('Choose a separate Sempai cache; refusing an existing/unknown source profile')
        finally:
            previous.close()
    snapshots = {}
    for relative in SOURCE_DOCUMENTS:
        path = (SOURCE_ROOT / relative).resolve()
        if not path.is_relative_to(SOURCE_ROOT) or not path.is_file():
            raise ValueError("Required local source missing or escapes package: " + relative)
        document = path.read_bytes().decode("utf-8")
        if not document.strip():
            raise ValueError("Required source is empty: " + relative)
        snapshots[relative] = document
    hashes = {name: digest(text) for name, text in snapshots.items()}
    # Catch a moving checkout. For release builds, supply the reviewed manifest too.
    for relative, expected in hashes.items():
        if digest((SOURCE_ROOT / relative).read_bytes().decode("utf-8")) != expected:
            raise ValueError("Source changed during snapshot: " + relative)
    pinned = os.getenv("SEMPAI_SOURCE_MANIFEST", "").strip()
    if pinned:
        supplied = json.loads(Path(pinned).read_text())
        if supplied.get("schema") != 1 or supplied.get("documents") != hashes:
            raise ValueError("Sempai source package differs from pinned manifest")
    snapshot = digest(json.dumps(hashes, sort_keys=True, separators=(",", ":")))
    revision = subprocess.run(["git", "-C", str(SOURCE_ROOT), "rev-parse", "HEAD"],
                              capture_output=True, text=True).stdout.strip()
    package = {"schema": 1, "documents": hashes, "snapshot": snapshot,
               "checkout_commit": revision or None,
               "approval": "reference snapshot only; not Q1/Q2 or runtime acceptance"}
    # Hashes, not the commit alone, identify dirty/local source content.
    for relative, document in snapshots.items():
        atomic_write(RAW / "documents" / (hashes[relative] + ".txt"), document)
    conn = db()
    changed = False
    def set_meta(key, value):
        conn.execute("INSERT INTO meta(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", (key, value))
    try:
        with conn:
            for relative, document in snapshots.items():
                metadata = {"url": "brassclaw-source:/" + relative,
                            "revision": snapshot, "snapshot": hashes[relative],
                            "version": "source package " + snapshot,
                            "applicability": SOURCE_DOCUMENTS[relative]}
                encoded = json.dumps(metadata, sort_keys=True)
                row = conn.execute("SELECT id,source_digest FROM documents WHERE repo=? AND path=?",
                                   ("brassclaw", relative)).fetchone()
                old_meta = meta_get(conn, "source:brassclaw:" + relative)
                if row is None:
                    changed = True
                    conn.execute("INSERT INTO documents(repo,path,text,source_digest) VALUES(?,?,?,?)",
                                 ("brassclaw", relative, document, hashes[relative]))
                elif row[1] != hashes[relative] or old_meta != encoded:
                    changed = True
                    invalidate_document(conn, row[0], "brassclaw", relative)
                    conn.execute("UPDATE documents SET text=?,source_digest=? WHERE id=?",
                                 (document, hashes[relative], row[0]))
                set_meta("source:brassclaw:" + relative, encoded)
            for doc_id, repo, relative in conn.execute("SELECT id,repo,path FROM documents").fetchall():
                if repo != "brassclaw" or relative not in snapshots:
                    changed = True
                    invalidate_document(conn, doc_id, repo, relative)
                    conn.execute("DELETE FROM documents WHERE id=?", (doc_id,))
            set_meta("sempai_source_package", json.dumps(package, sort_keys=True))
            set_meta("profile_id", "sempai")
            set_meta("documents", str(len(snapshots)))
    finally:
        conn.close()
    atomic_write(RAW / "source_manifest.json", json.dumps(package, indent=2, sort_keys=True) + "\n")
    atomic_write(DOCUMENTS_JSONL, "".join(json.dumps({"repo": "brassclaw", "path": name,
                  "sha256": hashes[name]}, sort_keys=True) + "\n" for name in sorted(snapshots)))
    if changed:
        invalidate_final()
    log(f"Pinned {len(snapshots)} complete documents; source snapshot {snapshot}; published generations retained")


def atoms_build():
    """Build atoms only for new/changed documents; unchanged checkpoints survive."""
    conn = db()
    n = 0
    for doc_id, repo, path, text, doc_digest in conn.execute("SELECT id,repo,path,text,source_digest FROM documents ORDER BY id"):
        existing = conn.execute("SELECT COUNT(*), COALESCE(MAX(source_digest),'') FROM atoms WHERE doc_id=?", (doc_id,)).fetchone()
        atom_count, atom_digest = existing
        expected_digest = digest(f"{ATOMIZER_VERSION}:{MICRO_TARGET}:{MICRO_MAX}:{TOKENIZER_MODEL}\n" + doc_digest)
        if atom_count and atom_digest == expected_digest:
            n += atom_count
            continue
        # Defensive invalidation if an older DB has stale atoms.
        invalidate_document(conn, doc_id, repo, path)
        invalidate_final()
        parts = atomize_text(text)
        rows = [(doc_id, i, ls, le, count(s), s, expected_digest) for i, (ls, le, s) in enumerate(parts)]
        conn.executemany("INSERT INTO atoms(doc_id,local_index,line_start,line_end,tokens,text,source_digest) VALUES(?,?,?,?,?,?,?)", rows)
        n += len(rows)
        conn.commit()
        log(f"Atoms {repo}/{path}: {len(rows):,}")
    conn.commit()
    meta_set(conn, "atoms", str(n))
    conn.close()
    log(f"Atoms: {n:,} (~{MICRO_TARGET} tokens, max {MICRO_MAX})")


# -----------------------------------------------------------------------------
# TRIAGE DRIVER
# -----------------------------------------------------------------------------

def _triage_flush(conn, batch: list[dict], counts: dict[str, int]) -> None:
    """Run only the cheap coarse route and checkpoint it atomically."""
    raw = stream_text(
        TRIAGE_PRE_SYSTEM,
        triage_prompt(batch),
        thinking=False,
        label=f"triage-pre {batch[0]['id']}-{batch[-1]['id']}",
        response_format={"type": "json_schema", "json_schema": {"name": "triage", "schema": triage_schema(len(batch))}},
    )
    masks = decode_masks(raw, len(batch))
    now = time.time()
    rows = []
    for a, mask in zip(batch, masks, strict=True):
        primary = mask_primary(mask)
        keep = int(mask != 0)
        rows.append((a["db_id"], mask, primary, keep, mask, now))
        if keep:
            for lane, bit in (("THINKING", 1), ("REASONING", 2), ("ACTION", 4)):
                if mask & bit:
                    counts[lane] += 1
        else:
            counts["NOISE"] += 1
    conn.executemany(
        "INSERT OR REPLACE INTO triage(atom_id,mask,primary_lane,keep,preliminary_mask,updated) VALUES(?,?,?,?,?,?)",
        rows,
    )
    conn.commit()


def atom_rows(conn, only_untriaged=True) -> Iterable[dict]:
    sql = """
    SELECT a.id,d.repo,d.path,a.line_start,a.line_end,a.tokens,a.text
    FROM atoms a JOIN documents d ON d.id=a.doc_id
    """
    if only_untriaged:
        sql += "LEFT JOIN triage t ON t.atom_id=a.id WHERE t.atom_id IS NULL "
    sql += "ORDER BY a.id"
    for row in conn.execute(sql):
        hint = heuristic_hint(row[2], row[6])
        yield {"id": f"{row[0]:08}", "db_id": row[0], "repo": row[1], "path": row[2], "line_start": row[3], "line_end": row[4], "tokens": row[5], "text": row[6], "local_hint": hint}


def triage():
    """One coarse pass only. Thinking happens later on coherent semantic groups."""
    conn = db()
    total = conn.execute("SELECT COUNT(*) FROM atoms").fetchone()[0]
    pending = conn.execute("SELECT COUNT(*) FROM atoms a LEFT JOIN triage t ON t.atom_id=a.id WHERE t.atom_id IS NULL").fetchone()[0]
    log(f"Triage: {total:,} atoms, {pending:,} pending")
    if TRIAGE_MODE not in {"local", "model"}:
        raise ValueError("TRIAGE_MODE must be local or model")
    if TRIAGE_MODE == "local":
        rows = []
        counts = Counter()
        for atom in atom_rows(conn, True):
            check_stop()
            mask = local_mask(atom["path"], atom["text"])
            lane = mask_primary(mask)
            counts[lane] += 1
            rows.append((atom["db_id"], mask, lane, int(mask != 0), mask, time.time()))
            if len(rows) >= 1000:
                conn.executemany("INSERT OR REPLACE INTO triage VALUES(?,?,?,?,?,?)", rows)
                conn.commit(); rows=[]
        if rows:
            conn.executemany("INSERT OR REPLACE INTO triage VALUES(?,?,?,?,?,?)", rows)
            conn.commit()
        conn.close()
        log(f"Local triage: {dict(counts)}; model calls=0")
        return

    local_rows: list[tuple] = []
    candidate_batch: list[dict] = []
    model_candidates = 0
    local_noise = 0
    batches = 0
    counts = {"THINKING": 0, "REASONING": 0, "ACTION": 0, "NOISE": 0}

    def flush_local() -> None:
        nonlocal local_rows
        if local_rows:
            conn.executemany(
                "INSERT OR REPLACE INTO triage(atom_id,mask,primary_lane,keep,preliminary_mask,updated) VALUES(?,?,?,?,?,?)",
                local_rows,
            )
            conn.commit()
            local_rows = []

    for atom in atom_rows(conn, True):
        check_stop()
        if atom["local_hint"] == "NOISE":
            local_noise += 1
            local_rows.append((atom["db_id"], 0, "NOISE", 0, 0, time.time()))
            if len(local_rows) >= 1000:
                flush_local()
            continue

        candidate_batch.append(atom)
        model_candidates += 1
        # Exact tokenizer packing; never build a second huge candidate list.
        candidate_body = triage_prompt(candidate_batch)
        if len(candidate_batch) > TRIAGE_MAX or chat_count(TRIAGE_PRE_SYSTEM, candidate_body, False) > int(REQUEST_INPUT_BUDGET * 0.82):
            last = candidate_batch.pop()
            if candidate_batch:
                _triage_flush(conn, candidate_batch, counts)
                batches += 1
            candidate_batch = [last]
            if chat_count(TRIAGE_PRE_SYSTEM, triage_prompt(candidate_batch), False) > REQUEST_INPUT_BUDGET:
                raise RuntimeError(f"Single atom exceeds REQUEST_INPUT_BUDGET: {last['id']}")
            if batches % 10 == 0:
                log(f"TRIAGE: batches={batches:,}; model_candidates={model_candidates:,}")

    if candidate_batch:
        _triage_flush(conn, candidate_batch, counts)
        batches += 1
    flush_local()
    conn.close()
    log(f"Local prefilter: {local_noise:,} obvious noise; model candidates={model_candidates:,}")
    log(f"Triage result: batches={batches:,}; coarse triage completed. NOISE={local_noise:,}")


# -----------------------------------------------------------------------------
# PASS 2: SEMANTIC GROUP -> THINK + SYNTHESIZE
# -----------------------------------------------------------------------------













def distill_lane(lane: str):
    if lane not in {"ACTION", "REASONING", "THINKING"}:
        raise ValueError("Unknown evidence lane")
    conn = db()
    try:
        with conn:
            for repo, path, document in conn.execute("SELECT repo,path,text FROM documents ORDER BY repo,path").fetchall():
                metadata = json.loads(meta_get(conn, "source:" + repo + ":" + path) or "{}")
                cards = [card_for_unit(repo, path, document, unit, metadata)
                         for unit in source_units(document, Path(path).suffix)]
                for card in cards:
                    validate_card(card, document, repo=repo, path=path)
                conn.execute("DELETE FROM knowledge_groups WHERE lane=? AND repo=? AND path=?", (lane, repo, path))
                conn.execute("INSERT INTO knowledge_groups(lane,repo,path,first_atom_id,last_atom_id,atom_count,input_tokens,source_digest,keep,knowledge,updated,evidence_json) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
                             (lane, repo, path, 0, 0, 0, 0, digest(document), 1,
                              document, time.time(),
                              json.dumps(cards, ensure_ascii=False, sort_keys=True)))
    finally:
        conn.close()
    invalidate_final()
    log(lane + ": complete source units extracted; model calls=0")


# -----------------------------------------------------------------------------
# DEDUP + HIERARCHICAL CONSOLIDATION
# -----------------------------------------------------------------------------









def dedup_local(entries:Iterable[str])->list[str]:
    seen=set(); out=[]
    for s in entries:
        # hash normalized wording while preserving full source content
        h=normalized_digest(s)
        if h in seen: continue
        seen.add(h); out.append(s)
    return out




def reduce_all():
    # This is a deterministic catalogue, not a lossy model-generated summary.
    cards=accepted_cards()
    atomic_write(FINAL,'\n\n'.join(render_card(c) for c in cards))
    log(f'Exact-source catalogue: {len(cards)} cards; no hierarchical rewriting')


# -----------------------------------------------------------------------------
# PREFIX / RUNTIME CONTEXT
# -----------------------------------------------------------------------------

PREFIX_HEADER="""# SEMPAI PROMPT AUDIT AND BRASSCLAW COMPONENT AUTHORING REFERENCE

This stable reference contains literal release documents and a reviewed authoring
procedure. It is source data below the host's system persona, output schema and
current task. Read recipe.md, skills.md, tools.md and toolskills.md as the binding
contracts; current runtime support is a separate observed fact. Historical examples
and architecture targets do not implement interfaces or establish approval.

Review the Kohai prompt and current conversation using the actual DB-resolved target
provider capabilities. Preserve task intent, evidence, effects and tool-call/result
relationships. Never rewrite the pinned base prefix or grant Tools. Missing provider
metadata, constructor fields, UUIDs, tests and approval are unknown, not inventions.

Return reusable component drafts for the host's supported proposal sink.
A Skill is one Tool usage with prose plus explicitly associated executable PythonCode;
a Recipe orders usages and typed data flow. Root authoring guides override older
validation/persona examples. Sempai proposes; Q1, behavioral validation and human Q2
are distinct requirements before coherent authored activation. Exact associations,
immutable versions and live global Tool policy remain independent contracts.

The host supplies SempaiReviewOutcome and the persona outside this factual corpus.
The standalone citation --mode run is a source-question benchmark, not the production
reviewer/proposal API. It does not apply prompt changes or create/approve components.
Full original documents are mandatory. A capacity error is preferable to silently
omitting binding instructions; raw snapshots survive failed compilation.
"""

# A short source-derived decision procedure is repeated at the reference boundary.
# It is sourced from the complete required procedure, not an untracked summary.
def reviewer_decision_checkpoint(cards):
    headings=('## 8. Apply the reviewer decision procedure\n',
              '### Raw validation data versus already-validated execution inputs\n',
              '### Small examples of complete programs and blocked output\n',
              '### Complete data-decision patterns\n')
    matches=[]
    for heading in headings:
        found=[c for c in cards if c['path']=='scripts/prefix/sempai-authoring-reference.md'
               and c['excerpt'].startswith(heading)]
        if len(found)!=1:
            raise ValueError('Missing exact Sempai reviewer decision procedure: '+heading.strip())
        matches.extend(found)
    if len(matches)!=len(headings):
        raise ValueError('Missing exact Sempai reviewer decision procedure')
    return '\n'.join(f"# REVIEWER DECISION CHECKPOINT — EXACT SOURCE {card['id'][:12]}\n"
                    +literal_block(card['excerpt'])+'\n' for card in matches)

REQUIRED_TOPICS = {
    "prompt-diagnosis": (("brassclaw",), r"sempai-authoring-reference\.md$", r"Diagnose the prompt"),
    "provider-authority": (("brassclaw",), r"simplified_v3\.md$", r"Provider ausschließlich in PostgreSQL"),
    "recipe-schema": (("brassclaw",), r"recipe\.md$", r"step_descriptions"),
    "typed-inputs": (("brassclaw",), r"recipe\.md$", r"inputs\[|typ.*input"),
    "skill-association": (("brassclaw",), r"skills\.md$", r"skill-association/1"),
    "association-approval": (("brassclaw",), r"skills\.md$", r"skill-association-approval/1"),
    "retry-and-effects": (("brassclaw",), r"skills\.md$", r"max_attempts"),
    "tool-primitives": (("brassclaw",), r"tools\.md$", r"Tool|primitive"),
    "toolskill-bindings": (("brassclaw",), r"toolskills\.md$", r"binding|Binding"),
    "q1-human-q2": (("brassclaw",), r"sempai-authoring-reference\.md$", r"human Q2"),
    "proposal-transport": (("brassclaw",), r"packet\.rs$", r"proposed_components"),
    "validation-queue": (("brassclaw",), r"14-validation-queue\.md$", r"Q1|Q2"),
    "no-self-activation": (("brassclaw",), r"sempai-authoring-reference\.md$", r"does\s+not\s+(?:write|mark validated)"),
}


def topic_reference_cards(cards):
    selected = {}
    for topic, (repos, path_pattern, text_pattern) in sorted(REQUIRED_TOPICS.items()):
        matches = [c for c in cards if c["repo"] in repos and re.search(path_pattern, c["path"], re.I)
                   and re.search(text_pattern, c["excerpt"], re.I)]
        if not matches:
            raise ValueError("Missing exact-source Sempai topic: " + topic)
        selected[topic] = min(matches, key=lambda c: c["id"])
    return selected


def required_reference_cards(cards):
    # Unlike domain packing, every allowlisted document/section is mandatory.
    # Prove complete line coverage against cached originals before token budgeting.
    conn = db()
    try:
        documents = {path: text for repo, path, text in conn.execute("SELECT repo,path,text FROM documents")
                     if repo == "brassclaw"}
        if set(documents) != set(SOURCE_DOCUMENTS):
            raise ValueError("Sempai source inventory is incomplete; recollect the full package")
        expected = {}
        for path, text in documents.items():
            for unit in source_units(text, Path(path).suffix):
                expected[(path, unit[0], unit[1])] = digest(text)
        actual = {(c["path"], c["line_start"], c["line_end"]): c["document_sha256"]
                  for c in cards if c["repo"] == "brassclaw"}
        if actual != expected or len(cards) != len(expected):
            raise ValueError("Sempai evidence omits or adds source units; complete originals are required")
        topic_reference_cards(cards)
        policy_path = os.getenv("SEMPAI_REFERENCE_POLICY", "").strip()
        if policy_path:
            policy = json.loads(Path(policy_path).read_text())
            if policy.get("schema") != 1 or policy.get("status") != "reviewed-source-selection":
                raise ValueError("Sempai requires an explicit reviewed source-selection policy")
            rows = policy.get("units")
            if not isinstance(rows, list) or len(rows) != len(expected):
                raise ValueError("Sempai selection must account for every original source unit")
            by_unit = {}
            for row in rows:
                key = (row["path"], row["line_start"], row["line_end"])
                if key in by_unit or type(row.get("selected")) is not bool:
                    raise ValueError("Invalid or duplicate Sempai selection unit")
                by_unit[key] = row
            if set(by_unit) != set(expected):
                raise ValueError("Sempai source-selection units drifted; review the new snapshot")
            selected = []
            complete = {"recipe.md", "skills.md", "tools.md", "toolskills.md",
                        "scripts/prefix/sempai-authoring-reference.md",
                        "crates/brassclaw_interceptor/src/packet.rs"}
            for card in cards:
                row = by_unit[(card["path"], card["line_start"], card["line_end"])]
                if (row.get("document_sha256") != card["document_sha256"]
                        or row.get("excerpt_sha256") != card["excerpt_sha256"]):
                    raise ValueError("Sempai source-selection fingerprints drifted; review the new snapshot")
                if card["path"] in complete and not row["selected"]:
                    raise ValueError("Sempai selection cannot omit complete binding guides/procedure/transport")
                if row["selected"]:
                    selected.append(card)
            topic_reference_cards(selected)
            cards = selected
        return sorted(cards, key=lambda c: (c["path"], c["line_start"], c["id"]))
    finally:
        conn.close()


def template_probe_tokens(tok, template, *, complete=True):
    """Render real request variants; deduplicate before exact batch tokenization.

    Never concatenate separately tokenized reference and suffix: BPE merges at
    their boundary would make the token count and cache-prefix claim unreliable.
    """
    requests=[]
    for thinking in (False,True):
        system='Operator instructions.'
        requests.append(([{'role':'system','content':system},{'role':'user','content':'CACHE-PROBE-A'}],None,thinking))
        if not complete: continue
        for content in (system,[{'type':'text','text':system}],None,''):
            for tool_name in (None,'read_configuration','list_interfaces'):
                tools=None if tool_name is None else [{'type':'function','function':{
                    'name':tool_name,'description':'Read-only diagnostic '+tool_name,
                    'parameters':{'type':'object','properties':{}}}}]
                requests.append(([{'role':'system','content':content},
                                  {'role':'user','content':'CACHE-PROBE-B'}],tools,thinking))
        requests.append(([{'role':'user','content':'CACHE-PROBE-A'}],None,thinking))
        requests.append(([{'role':'user','content':[{'type':'text','text':'CACHE-PROBE-B'}]}],None,thinking))
        requests.append(([{'role':'system','content':system},{'role':'user','content':'Earlier question'},
                          {'role':'assistant','content':'Earlier answer'},{'role':'user','content':'CACHE-PROBE-B'}],None,thinking))
    rendered=list(dict.fromkeys(tok.apply_chat_template(messages,tools=tools,chat_template=template,
        tokenize=False,add_generation_prompt=True,enable_thinking=thinking)
        for messages,tools,thinking in requests))
    return tok(rendered,add_special_tokens=False,return_attention_mask=False)['input_ids']


def build_context(target:int=CONTEXT_TARGET_TOKENS, *, offline=False):
    profile=model_profile(verify_server=not offline); cards=accepted_cards(); tok=tokenizer()
    reserve=int(os.getenv('PREFIX_WORKSPACE_RESERVE_TOKENS','31072'))
    if reserve<8192: raise ValueError('Reserve at least 8192 tokens for task, evidence, reasoning and output')
    budget=min(int(target),profile['max_model_len']-reserve)
    if budget<=0: raise ValueError('No room for a reference prefix')
    policy_path=os.getenv('SEMPAI_REFERENCE_POLICY','').strip()
    policy_text=Path(policy_path).read_text() if policy_path else None
    policy_sha256=digest(policy_text) if policy_text is not None else None
    header=PREFIX_HEADER+'\n# EXACT-SOURCE REFERENCE\n\n'
    if policy_text is not None:
        header=header.replace('Full original documents are mandatory.',
            'Full raw originals are mandatory; model-visible coverage follows the reviewed source-selection policy.')
        header+='Selected architecture excerpts are not complete copies of those documents. Scoped storage columns/view filters are not additional v3 authority checks. Transport structs do not establish supported component insertion constructors.\n\n'
    mandatory=required_reference_cards(cards); mandatory_ids={c["id"] for c in mandatory}
    checkpoint=reviewer_decision_checkpoint(mandatory)
    header+=checkpoint+'\n'
    anchors=[]
    for repo in ():  # No repeated diagnostic anchors: all required source units are already present.
        match=next((c for c in mandatory if c["repo"]==repo),None)
        if match is not None: anchors.append(match)
    selected=list(mandatory); omitted=[c['id'] for c in cards if c['id'] not in mandatory_ids] if policy_text is not None else []
    rendered={}
    def rendered_card(card):
        if card['id'] not in rendered:
            rendered[card['id']]=(f"<!-- EVIDENCE-CARD {card['id'][:12]} -->\n"
                f"## {transport_text(card['path'])} : L{card['line_start']}-{card['line_end']}\n"
                +literal_block(card['excerpt'])+'\n<!-- END-EVIDENCE-CARD -->'
                if policy_text is not None else render_card(card))
        return rendered[card['id']]
    long_contexts={digest(value.strip()) for c in mandatory for value in [c["parent_headings"],c["excerpt"]] if len(value)>2000}
    used=count(header)+sum(count(rendered_card(c)) for c in mandatory+anchors)
    if used > budget:
        raise ValueError(f'Capacity blocked: complete Sempai evidence needs approximately {used} tokens before the final envelope; budget={budget}. Preserve originals and use a compatible larger context or a reviewed source policy; no guides were truncated.')
    # Greedy selection is deterministic and atomic at card boundaries. Exact
    # rendered-template accounting below is authoritative, not the token estimate.
    ordered_cards=[] if policy_text is not None else [c for c in balanced_cards(cards) if c["id"] not in mandatory_ids]
    for position,card in enumerate(ordered_cards):
        check_stop()
        if used >= budget-1024:
            omitted.extend(c["id"] for c in ordered_cards[position:]); break
        if re.search(r"(?:^|/)(?:releases?|preface|changelog|news)(?:/|\.)|(?:^|/)HISTORY\.",card["path"],re.I):
            omitted.append(card["id"]); continue
        text=rendered_card(card); cost=count('\n\n'+text)
        context_key=digest(card["parent_headings"].strip()) if len(card["parent_headings"])>2000 else None
        if (context_key and context_key in long_contexts) or digest(card["excerpt"].strip()) in long_contexts:
            omitted.append(card["id"]); continue
        if cost>int(os.getenv("PREFIX_MAX_CARD_TOKENS","12000")):
            omitted.append(card["id"]); continue
        if used+cost<=budget-512:
            selected.append(card); used+=cost
            if context_key: long_contexts.add(context_key)
            if len(card["excerpt"])>2000: long_contexts.add(digest(card["excerpt"].strip()))
        else: omitted.append(card['id'])
    if not selected: raise ValueError('No complete evidence card fits prefix budget')
    original=tok.chat_template
    if isinstance(original,dict): original=original.get('default')
    if not isinstance(original,str): raise ValueError('Missing model chat template')
    def artifacts(chosen):
        coverage=Counter(c['repo'] for c in chosen)
        index='\n'.join(f"- {repo}: {PLATFORM_TAGS.get(repo, 'source-specific')} | {coverage[repo]} complete cards" for repo in sorted(coverage))
        locators='\n'.join(f"- {c['id'][:12]} | {c['repo']} | {card_title(c)}" for c in chosen)
        documents={c['path']:c for c in chosen}
        provenance='\n'.join(f"- {path} | SHA256 {c['document_sha256']} | {c['source']['url']} | {c['source'].get('applicability','source-specific')}"
            for path,c in sorted(documents.items())) if policy_text is not None else ''
        full=header+'# PLATFORM COVERAGE\n'+transport_text(index)+'\n\n# SOURCE SNAPSHOTS\n'+transport_text(provenance)+'\n\n# CARD INDEX — SOURCE TITLES\n'+transport_text(locators)+'\n\n'+'\n\n'.join(rendered_card(c) for c in chosen)
        full+='\n\n# DIAGNOSTIC ANCHORS — EXACT SOURCE REPEATED NEAR TASK\n\n'+'\n\n'.join(rendered_card(c) for c in anchors)
        full+='\n\n'+checkpoint+'\n# FACTUAL CHECK BEFORE ANSWERING\n'
        full+='Review target provider metadata and the actual Kohai packet. Preserve volatile-tail roles, tool relationships and task evidence. Propose reusable Recipes with supported fields and real catalogue/draft identities. The host output schema is separate. Q1, observed behavior, human Q2 and exact association approval precede activation; never self-approve or replay completed effects. Missing runtime support remains an explicit prerequisite.\n'
        # The deployed renderer may convert scalar text into OpenAI text parts.
        # Merge into the existing system message in either representation.
        # A non-whitespace boundary prevents the model template's strip() from
        # removing the reference's final newline for absent/empty system content.
        wrapper='{% set _engineering_reference = '+json.dumps(full+'\n\n# REQUEST CONTEXT\n',ensure_ascii=False)+' %}\n'
        wrapper+="""{%- if not (tools and tools is iterable and tools is not mapping) -%}
{%- if messages and messages[0]['role'] == 'system' -%}
{%- if messages[0]['content'] is string -%}
{%- set _system_content = _engineering_reference ~ '\n\n' ~ messages[0]['content'] -%}
{%- elif messages[0]['content'] is iterable and messages[0]['content'] is not mapping -%}
{%- set _system_content = [{'type':'text','text':_engineering_reference ~ '\n\n'}] + messages[0]['content'] -%}
{%- elif messages[0]['content'] is none -%}
{%- set _system_content = _engineering_reference -%}
{%- else -%}
{{- raise_exception('Unexpected system content type.') -}}
{%- endif -%}
{%- set messages = [{'role':'system','content':_system_content}] + messages[1:] -%}
{%- else -%}
{%- set messages = [{'role':'system','content':_engineering_reference}] + messages -%}
{%- endif -%}
{%- endif -%}
"""
        # Original Qwen puts changing tools before system content. Insert the
        # static reference immediately after that system opening instead.
        opening="{{- '<|im_start|>system\\n' }}"
        if original.count(opening)!=1:
            raise ValueError('Deployed chat template tool-system opening changed; inspect before exporting')
        tool_first=original.replace(opening,"{{- '<|im_start|>system\\n' + _engineering_reference + '\\n\\n' }}",1)
        return full,wrapper+tool_first
    # Only two probes are needed during budget trimming. The full render/token
    # matrix is checked once per candidate that fits, with exact BPE accounting.
    while selected:
        full,template=artifacts(selected)
        sequences=template_probe_tokens(tok,template,complete=False)
        total=max(map(len,sequences))
        if total<=budget:
            sequences=template_probe_tokens(tok,template,complete=True)
            total=max(map(len,sequences))
            if total<=budget: break
        if selected[-1]['id'] in mandatory_ids:
            raise ValueError('Prefix budget cannot fit mandatory complete evidence cards and anchors')
        omitted.append(selected.pop()['id'])
    if not selected: raise ValueError('No complete card fits the rendered model template')
    common=0
    for tokens in zip(*sequences):
        if len(set(tokens))!=1: break
        common+=1
    block=profile['cache_block_tokens']
    if block<=0: raise ValueError('Invalid cache block size')
    # A stable system opening alone is not evidence that the reference itself
    # is cacheable. Demand the entire literal reference in the shared prefix.
    if not tok.decode(sequences[0][:common],skip_special_tokens=False).startswith('<|im_start|>system\n'+full):
        raise ValueError(f'Reference is not a complete stable leading token prefix across supported request shapes ({common} shared tokens)')
    if len({c["id"][:12] for c in selected}) != len(selected):
        raise ValueError("Compact card identifiers collided")
    evidence_records=''.join(json.dumps(c,ensure_ascii=False,sort_keys=True)+'\n' for c in selected)
    # Provenance can change without changing rendered words. Bind the complete
    # evidence payload as well, otherwise an immutable directory could collide.
    evidence_sha256=digest(evidence_records)
    compiler_sha256=file_sha256(Path(__file__))
    if policy_path and Path(policy_path).read_text()!=policy_text:
        raise ValueError('Sempai source-selection policy changed during compilation')
    generation=digest(json.dumps([PIPELINE_VERSION,compiler_sha256,profile,full,template,evidence_sha256,policy_sha256],sort_keys=True))
    directory=BASE/'generations'/generation; directory.mkdir(parents=True,exist_ok=True)
    manifest={'schema':2,'generation':generation,'profile_id':'sempai',
              'compiler_sha256':compiler_sha256,'model_profile':profile,
              'source_selection_policy_sha256':policy_sha256,
              'source_selection_mode':'reviewed-complete-units' if policy_text is not None else 'complete-originals',
              'sha256':digest(full),'text_tokens':count(full),'rendered_probe_tokens':total,
              'prefix_token_budget':budget,'workspace_reserve_tokens':reserve,
              'shared_prefix_tokens':common,'complete_cache_blocks':common//block,
              'shared_prefix_token_sha256':digest(json.dumps(sequences[0][:common],separators=(',',':'))),
              'template_probe_count':len(sequences),
              'external_cache_reuse_verified':False,
              'evidence_cards_sha256':evidence_sha256,
              'selected_card_ids':[c['id'] for c in selected],
              'cacheable_shared_tokens':common//block*block,'uncached_tail_tokens':common%block,
              'near_task_anchor_card_ids':[c['id'] for c in anchors],
              'required_sources':sorted({c['repo'] for c in mandatory}),
              'required_domains':sorted({c['domain'] for c in mandatory}),
              'required_topic_card_ids':{topic:c['id'] for topic,c in topic_reference_cards(mandatory).items()},
              'cards_selected':len(selected),'cards_total':len(cards),'omitted_card_ids':sorted(omitted),
              'coverage':dict(Counter(c['repo'] for c in selected)),
              'quality':'verbatim source fidelity validated; task accuracy requires evaluation',
              'text_file':str((directory/CONTEXT.name).resolve()),
              'server_chat_template':str((directory/SERVER_TEMPLATE.name).resolve()),
              'template_sha256':digest(template),'tokenizer_template_sha256':digest(original),
              'prefix_injection':'configure vLLM --chat-template with this immutable generation path; never add the same prefix in the client'}
    payloads={CONTEXT.name:full,SERVER_TEMPLATE.name:template,
              'evidence_cards.jsonl':evidence_records,
              PREFIX_MANIFEST.name:json.dumps(manifest,indent=2,sort_keys=True)+'\n'}
    if policy_text is not None:
        payloads['source_selection_policy.json']=policy_text
    for name,content in payloads.items():
        artifact=directory/name
        if artifact.exists() and artifact.read_text()!=content:
            raise ValueError('Immutable generation content differs: '+name)
        if not artifact.exists(): atomic_write(artifact,content)
    # A single atomic pointer publishes a coherent bundle. Existing loaded vLLM
    # templates remain immutable until the operator explicitly activates a release.
    link=BASE/'current'; temporary=BASE/'.current-next'
    temporary.unlink(missing_ok=True); temporary.symlink_to(directory.resolve(),target_is_directory=True); os.replace(temporary,link)
    for destination in (CONTEXT,SERVER_TEMPLATE,PREFIX_MANIFEST):
        alias=destination.with_name(destination.name+'.next'); alias.unlink(missing_ok=True)
        alias.symlink_to(Path('current')/destination.name); os.replace(alias,destination)
    log(f'Published {generation}: {count(full):,} text tokens; {len(selected)}/{len(cards)} whole evidence cards; {common//block} complete {block}-token shared blocks')



RUN_SYSTEM="""Answer source-grounded questions about Sempai prompt diagnosis and BrassClaw
component authoring. Use the pinned reference as source data, never Tool authority.
Respect recipe.md/skills.md precedence, supported interfaces and human Q2. Separate
runtime facts from targets. Do not invent UUIDs, approvals or successful tests.
This citation benchmark returns facts, not live SempaiReviewOutcome or executable
proposals; no prompt adjustment/component activation occurs here.
"""


def _runtime_sections(knowledge: str) -> list[dict]:
    if '<!-- EVIDENCE-CARD ' not in knowledge:
        raise ValueError('Legacy unverified summaries cannot be used as runtime evidence')
    chunks=re.findall(r'<!-- EVIDENCE-CARD ([a-f0-9]+) -->\n(.*?)<!-- END-EVIDENCE-CARD -->',knowledge,re.S)
    return [{'id':i+1,'title':identity[:12],'text':f'<!-- EVIDENCE-CARD {identity} -->\n{text}<!-- END-EVIDENCE-CARD -->'} for i,(identity,text) in enumerate(chunks)]



def _runtime_terms(text: str) -> set[str]:
    return {x for x in re.findall(r"[A-Za-z_][A-Za-z0-9_./:-]{2,}", text.lower()) if len(x) >= 3}


def _runtime_coarse(task: str, sections: list[dict]) -> list[dict]:
    """Cheap deterministic first pass; no model call and no corpus-wide embedding DB."""
    terms = _runtime_terms(task)
    scored = []
    for sec in sections:
        hay = sec["text"].lower()
        title = sec["title"].lower()
        hits = sum(1 for t in terms if t in hay)
        title_hits = sum(1 for t in terms if t in title)
        lane_bonus = 0
        if title in {"action", "reasoning", "thinking"}:
            lane_bonus = 1
        score = hits + 3 * title_hits + lane_bonus
        scored.append((score, -sec["id"], sec))
    scored.sort(reverse=True, key=lambda x: (x[0], x[1]))
    return [x[2] for x in scored[:max(1, RUNTIME_COARSE_CANDIDATES)]]


def search_knowledge(task: str) -> list[dict]:
    """Index source documents first; validate only candidate evidence cards.

    The index is a shortlist, never acceptance evidence. Exact-source validation
    still runs on every returned card, with platform filtering before ranking.
    """
    allowed=task_platforms(task); terms=_runtime_terms(task)
    ignored={'the','and','for','with','this','that','how','can','you','please','what','from'}
    query_terms=sorted(terms-ignored)[:32]
    if not query_terms: return []
    query=' OR '.join('"'+term.replace('"','""')+'"' for term in query_terms)
    conn=db(); params=[query]; scope=''
    if allowed:
        scope=' AND g.repo IN ('+','.join('?' for _ in allowed)+')'
        params.extend(sorted(allowed))
    sql="""SELECT g.repo,g.path,g.evidence_json,d.text
        FROM knowledge_search s JOIN knowledge_groups g ON g.id=s.rowid
        JOIN documents d ON d.repo=g.repo AND d.path=g.path
        WHERE knowledge_search MATCH ? AND g.keep=1 AND g.updated>0
        AND g.evidence_json<>''"""+scope+" ORDER BY bm25(knowledge_search,1.0,2.0,5.0),g.id LIMIT 48"
    rows=conn.execute(sql,params).fetchall()
    # Broad lexical tasks must not bury curated diagnostic pages in the FTS
    # shortlist. Include those source documents for explicitly requested platforms.
    seeded_paths=[]
    for repo in sorted(allowed):
        for url in WEB_SOURCES.get(repo,{}).get('seeds',[]):
            seeded_paths.append(urlparse(url).path.strip('/')+'.md')
    if seeded_paths:
        extra_sql="""SELECT g.repo,g.path,g.evidence_json,d.text FROM knowledge_groups g
            JOIN documents d ON d.repo=g.repo AND d.path=g.path
            WHERE g.keep=1 AND g.updated>0 AND g.evidence_json<>''
            AND g.repo IN ("""+','.join('?' for _ in allowed)+") AND REPLACE(g.path,'/.md','.md') IN ("+','.join('?' for _ in seeded_paths)+")"
        rows+=conn.execute(extra_sql,sorted(allowed)+seeded_paths).fetchall()
    conn.close()
    ranked=[]; seen=set()
    for repo,path,raw,document in rows:
        for card in json.loads(raw):
            if card['id'] in seen: continue
            seen.add(card['id'])
            hay=(card['path']+' '+card['parent_headings']+' '+card['excerpt']).lower()
            score=sum(1 for term in query_terms if term in hay)
            if score:
                if card["source"].get("url", "").rstrip("/") in {u.rstrip("/") for u in WEB_SOURCES.get(repo,{}).get("seeds",[])}: score+=2
                if "```" in card["excerpt"]: score+=2
                validate_card(card,document,repo=repo,path=path)
                ranked.append((score,card['id'],card))
    ranked.sort(key=lambda item:(-item[0],item[1]))
    return [{'id':index+1,'evidence_id':card['id'],'title':card['repo']+'::'+card['path'],'text':render_card(card)}
            for index,(_,_,card) in enumerate(ranked[:RUNTIME_RERANK_KEEP])]










def build_runtime_context(task: str, target: int = RUNTIME_CONTEXT_TOKENS) -> str:
    ordered=search_knowledge(task)
    if not ordered: raise RuntimeError('No applicable accepted evidence for this task; inspect platform/version and source coverage')
    overhead=chat_count(effective_system(RUN_SYSTEM),'',True)-chat_count(RUN_SYSTEM,'',True)
    budget=min(int(target),MODEL_CONTEXT_LIMIT-int(os.getenv('TASK_OUTPUT_RESERVE_TOKENS','8192'))-overhead)
    active_ids=set(re.findall(r'<!-- EVIDENCE-CARD ([a-f0-9]+) -->',active_prefix_text()))
    already=[s['evidence_id'] for s in ordered if s['evidence_id'][:12] in active_ids]
    ordered=[s for s in ordered if s['evidence_id'][:12] not in active_ids]
    prefix=PREFIX_HEADER+'\n# TASK EVIDENCE\n\n'; suffix='\n\n--- USER TASK ---\n'+task; picked=[]
    for section in ordered:
        trial=prefix+'\n\n'.join(picked+[section['text']])+suffix
        if chat_count(RUN_SYSTEM,trial,True)<=budget: picked.append(section['text'])
    if not picked and already:
        return 'Applicable evidence is in the active server prefix. Card IDs: '+', '.join(already)
    if not picked: raise ValueError('No complete applicable evidence card fits task context')
    return prefix+'\n\n'.join(picked)




def run_task(task: str) -> str:
    from prefix_response_validation import run_compiler_task
    return run_compiler_task(task, system=RUN_SYSTEM, server=VLLM_URL,
                             prefix_file=ACTIVE_SERVER_PREFIX, verify_local=active_prefix_text,
                             stream=stream_text, kind="none",
                             receipt=os.getenv("PREFIX_RESPONSE_RECEIPT"))


def plan():
    log("Sempai offline source inventory; no model calls or source mutations")
    total = 0
    for relative, applicability in SOURCE_DOCUMENTS.items():
        path = SOURCE_ROOT / relative
        size = path.stat().st_size if path.is_file() else None
        total += size or 0
        log(f"{relative}: {size if size is not None else 'MISSING'} bytes | {applicability}")
    log(f"Full source bytes: {total}; exact rendered token/capacity checks happen at --mode context --offline")
    log("All documents/units are mandatory; no token padding or silent coverage reduction")

# -----------------------------------------------------------------------------
# STATUS / DOCTOR / RESET
# -----------------------------------------------------------------------------

def status():
    conn=db()
    docs=conn.execute("SELECT COUNT(*) FROM documents").fetchone()[0]
    atoms_n=conn.execute("SELECT COUNT(*) FROM atoms").fetchone()[0]
    tri=conn.execute("SELECT COUNT(*) FROM triage").fetchone()[0]
    kept=conn.execute("SELECT COUNT(*) FROM triage WHERE keep=1").fetchone()[0]
    know=conn.execute("SELECT COUNT(*) FROM knowledge_groups WHERE keep=1 AND knowledge<>''").fetchone()[0]
    log("\n"+"="*72)
    log("ORNITH KNOWLEDGE PIPELINE STATUS")
    log("="*72)
    log(f"Model          : {MODEL}")
    log(f"Tokenizer      : {TOKENIZER_MODEL}")
    log(f"vLLM           : {VLLM_URL}")
    log(f"Micro          : {MICRO_TARGET} / max {MICRO_MAX}")
    groups=conn.execute("SELECT COUNT(*) FROM knowledge_groups").fetchone()[0]
    log(f"Request input  : {REQUEST_INPUT_BUDGET}")
    log(f"Semantic groups: {groups:,}")
    log(f"Docs           : {docs:,}")
    log(f"Atoms          : {atoms_n:,}")
    log(f"Triaged        : {tri:,}")
    log(f"Kept           : {kept:,}")
    log(f"Knowledge      : {know:,}")
    log(f"Final KB       : {'YES' if FINAL.exists() else 'NO'}")
    log(f"Context        : {'YES' if CONTEXT.exists() else 'NO'}")
    if FINAL.exists(): log(f"Final tokens    : {count(FINAL.read_text(encoding='utf-8')):,}")
    conn.close(); log("="*72)


def doctor():
    cards = accepted_cards()
    required_reference_cards(cards)
    log(f"Verified {len(cards)} complete exact-source units; tokenizer/server/cache not tested")


def reset():
    if BASE.exists():
        log(f"Deleting {BASE} ...")
        shutil.rmtree(BASE)
    BASE.mkdir(parents=True,exist_ok=True)
    log("Reset complete.")

# -----------------------------------------------------------------------------
# PIPELINE / CLI
# -----------------------------------------------------------------------------

@lru_cache(maxsize=64)
def source_units(text: str, suffix: str = '.md') -> list[tuple[int, int, str, str]]:
    """Complete documentation sections; executable files remain whole.

    Literal parent preambles travel with children. If these are too large, the
    complete card is omitted from the bounded prefix rather than trimming its
    qualifications. RST literal blocks are indented and cannot become headings.
    """
    lines=text.splitlines(keepends=True)
    if suffix not in {'.md','.markdown','.mdx','.rst','.adoc','.txt'}:
        return [(1,len(lines),text,'')] if lines else []
    boundaries=[0]; parents={}; stack=[]; fence=None; rst_levels={}
    for index,line in enumerate(lines):
        heading=None
        if suffix=='.rst':
            if (line.strip() and not line[0].isspace() and index+1<len(lines)
                and re.fullmatch(r'([=\-~^"\x27`:+*#_])\1{3,}\s*',lines[index+1])
                and len(lines[index+1].strip())>=len(line.strip())):
                marker=lines[index+1].strip()[0]
                if marker not in rst_levels: rst_levels[marker]=len(rst_levels)+1
                heading=rst_levels[marker]
        else:
            marker=re.match(r'^\s{0,3}(`{3,}|~{3,})',line)
            if marker:
                value=marker.group(1)
                if fence is None: fence=value
                elif value[0]==fence[0] and len(value)>=len(fence): fence=None
                continue
            match=re.match(r'^(#{1,6})\s+',line) if fence is None else None
            if suffix=='.adoc': match=re.match(r'^(={1,6})\s+',line) if fence is None else None
            if match: heading=len(match.group(1))
        if heading:
            stack=[(depth,start) for depth,start in stack if depth<heading]
            parents[index]=[start for _,start in stack]; stack.append((heading,index))
            if index: boundaries.append(index)
    boundaries.append(len(lines)); ends=dict(zip(boundaries,boundaries[1:]))
    return [(start+1,end,''.join(lines[start:end]),
             '\n'.join(''.join(lines[parent:ends[parent]]) for parent in parents.get(start,[])))
            for start,end in zip(boundaries,boundaries[1:]) if end>start]




def card_for_unit(repo, path, document, unit, metadata):
    start, end, excerpt, parents = unit
    snapshot = digest(document)
    identity = digest(json.dumps([repo,path,snapshot,start,end],ensure_ascii=False))
    return {'schema': 1, 'id': identity, 'repo': repo, 'path': path,
            'domain': REPO_DOMAINS.get(repo, 'engineering'),
            'platform': PLATFORM_TAGS.get(repo, 'source-specific; determine from excerpt'),
            'version': str(metadata.get('version','unspecified in source metadata')),
            'document_sha256': snapshot, 'excerpt_sha256': digest(excerpt),
            'context_sha256': digest(parents),
            'line_start': start, 'line_end': end, 'parent_headings': parents,
            'excerpt': excerpt, 'source': metadata, 'status': 'exact-source-excerpt'}



@lru_cache(maxsize=64)
def source_unit_index(document, suffix):
    return {(start,end):parents for start,end,_,parents in source_units(document,suffix)}


def validate_card(card, document, *, repo=None, path=None):
    if card.get('schema') != 1 or card.get('status') != 'exact-source-excerpt':
        raise ValueError('Unsupported or unverified evidence card')
    lines = document.splitlines(keepends=True)
    start, end = card['line_start'], card['line_end']
    if type(start) is not int or type(end) is not int:
        raise ValueError('Evidence line ranges must be integers')
    if not 1 <= start <= end <= len(lines): raise ValueError('Invalid evidence range')
    exact = ''.join(lines[start-1:end])
    if (digest(document) != card['document_sha256'] or exact != card['excerpt']
            or digest(exact) != card['excerpt_sha256']):
        raise ValueError('Evidence does not match its source snapshot')
    identity=digest(json.dumps([card['repo'],card['path'],card['document_sha256'],start,end],ensure_ascii=False))
    if card['id']!=identity or (repo is not None and card['repo']!=repo) or (path is not None and card['path']!=path):
        raise ValueError('Evidence identity or source document binding differs')
    source=card.get('source')
    if not isinstance(source,dict) or not source.get('url') or not (source.get('revision') or source.get('snapshot')):
        raise ValueError('Evidence requires a source URL and immutable revision/snapshot')
    if source.get('snapshot') and source['snapshot']!=card['document_sha256']:
        raise ValueError('Source snapshot differs from evidence document')
    if (card['domain']!=REPO_DOMAINS.get(card['repo'],'engineering')
        or card['platform']!=PLATFORM_TAGS.get(card['repo'],'source-specific; determine from excerpt')
        or card['version']!=str(source.get('version','unspecified in source metadata'))):
        raise ValueError('Evidence applicability metadata differs from its source family')
    if 'context_sha256' in card and card['context_sha256']!=digest(card['parent_headings']):
        raise ValueError('Evidence parent context fingerprint differs')
    expected = source_unit_index(document,Path(card['path']).suffix).get((start,end))
    if expected is None or expected != card['parent_headings']:
        raise ValueError('Evidence context is not a complete source unit')



def transport_text(text):
    # Keep originals in evidence JSON. Render chat-control tokens as visible escapes
    # so collected text cannot close a message in the model's chat serialization.
    # Qwen tokenizers can omit im_start from all_special_tokens while still
    # encoding it as a single chat delimiter. Cover the serialized spelling.
    text = re.sub(r'<\|[^<>\r\n]{1,128}\|>',
                  lambda match: match.group().replace('<', r'\u003c').replace('>', r'\u003e'), text)
    text = re.sub(r'<!--\s*(?:END-)?EVIDENCE-CARD\b[^\r\n]*?-->',
                  lambda match: match.group().replace('<',r'\u003c').replace('>',r'\u003e'),text)
    for token in sorted(tokenizer().all_special_tokens, key=len, reverse=True):
        if token and token in text:
            text = text.replace(token, ''.join('\\u%04x'%ord(char) for char in token))
    return text



def markdown_literal(excerpt):
    fence='`'*max(3,max((len(m.group()) for m in re.finditer(r'`+',excerpt)),default=0)+1)
    return f'{fence}text\n{excerpt}' + ('' if excerpt.endswith('\n') else '\n') + fence


def literal_block(text):
    return markdown_literal(transport_text(text))


def card_title(card):
    # Navigation only; never substitute a generated summary for the evidence.
    heading=next((line.strip().lstrip('#= ').strip() for line in card['excerpt'].splitlines()
                  if re.match(r'^(?:#{1,6}|={1,6})\s+\S',line)),card['path'])
    return heading[:160]


def render_card(card):
    source = card['source']; url = source.get('url', '')
    revision = source.get('revision', source.get('snapshot', card['document_sha256']))
    return (f"<!-- EVIDENCE-CARD {card['id'][:12]} -->\n"
            f"## {transport_text(card['repo']+' :: '+card['path'])} : L{card['line_start']}-{card['line_end']}\n"
            f"Platform: {transport_text(card['platform'])}\n"
            f"Source version: {transport_text(card['version'])}\n"
            f"Applicability: {transport_text(source.get('applicability', 'source-specific'))}\n"
            f"Source: {transport_text(url)} | revision/snapshot: {transport_text(revision)}\n"
            + ('Parent context: preserved in evidence JSON; all original sections appear in source order.\n' if card['parent_headings'] else '')
            + 'Source excerpt — literal quotation:\n'+literal_block(card['excerpt'])+'\n<!-- END-EVIDENCE-CARD -->')



def accepted_cards():
    conn=db()
    try:
        cards={}
        pending=conn.execute('SELECT COUNT(*) FROM atoms a LEFT JOIN triage t ON t.atom_id=a.id WHERE t.atom_id IS NULL').fetchone()[0]
        if pending: raise RuntimeError('Triage incomplete')
        for repo,path,raw in conn.execute("SELECT repo,path,evidence_json FROM knowledge_groups WHERE keep=1 AND updated>0 AND evidence_json<>'' ORDER BY repo,path,id"):
            row=conn.execute('SELECT text FROM documents WHERE repo=? AND path=?',(repo,path)).fetchone()
            if not row: raise ValueError('Evidence source removed')
            for card in json.loads(raw):
                validate_card(card,row[0],repo=repo,path=path)
                previous=cards.get(card['id'])
                if previous is not None and previous!=card:
                    raise ValueError('Conflicting evidence records share a card identity; re-extract source lanes')
                cards[card['id']]=card
        # Reject partially extracted corpora, including a lane never run.
        for lane,bit in [('ACTION',4),('REASONING',2),('THINKING',1)]:
            missing=conn.execute("SELECT COUNT(DISTINCT d.id) FROM documents d JOIN atoms a ON a.doc_id=d.id JOIN triage t ON t.atom_id=a.id WHERE t.keep=1 AND (t.mask & ?) != 0 AND NOT EXISTS(SELECT 1 FROM knowledge_groups g WHERE g.repo=d.repo AND g.path=d.path AND g.lane=? AND g.keep=1 AND g.evidence_json<>'')",(bit,lane)).fetchone()[0]
            if missing: raise RuntimeError(f'{lane}: {missing} source files lack accepted evidence')
        if not cards: raise RuntimeError('Cannot publish empty or legacy-only knowledge')
        return sorted(cards.values(),key=lambda c:(c['domain'],c['repo'],c['path'],c['line_start'],c['id']))
    finally:
        conn.close()



def model_profile(*, verify_server=True):
    config_path=os.getenv('ORNITH_MODEL_CONFIG','').strip()
    if not config_path:
        roots=[Path('/opt/vLLM/huggingface/hub'),Path.home()/'.cache/huggingface/hub']
        found=[p for root in roots for p in root.glob('models--'+MODEL.replace('/','--')+'/snapshots/*/config.json')]
        if len(found)!=1: raise RuntimeError('Set ORNITH_MODEL_CONFIG to the deployed immutable model config')
        config_path=str(found[0])
    path=Path(config_path).resolve(); raw=path.read_text(); config=json.loads(raw)
    tokenizer_files={name:file_sha256(path.parent/name) for name in
                     ['tokenizer.json','tokenizer_config.json','chat_template.jinja',
                      'special_tokens_map.json','vocab.json','merges.txt'] if (path.parent/name).is_file()}
    if not {'tokenizer.json','tokenizer_config.json'}<=tokenizer_files.keys():
        raise ValueError('Immutable tokenizer artifacts are missing beside the model config')
    text=config.get('text_config',config); layers=text.get('layer_types',[])
    if layers.count('linear_attention')!=24 or layers.count('full_attention')!=8:
        raise ValueError('Model profile differs from the supported Ornith hybrid architecture')
    if verify_server:
        headers={'Authorization':'Bearer '+os.environ['VLLM_API_KEY']} if os.getenv('VLLM_API_KEY') else {}
        response=httpx.get(VLLM_URL.rstrip('/')+'/models',headers=headers,timeout=15); response.raise_for_status()
        served=next((m for m in response.json()['data'] if m['id']==MODEL),None)
        if not served or not served.get('max_model_len'): raise RuntimeError('Serving model/context limit not verified')
        runtime = serving_configuration()
    else:
        # A local compiler preview, not a claim about a running server. Avoid
        # /proc scanning and network calls, and make the cache assumptions visible.
        served={'max_model_len':min(MODEL_CONTEXT_LIMIT,text['max_position_embeddings'])}
        runtime={'--kv-cache-dtype':'fp8'}
    kv_dtype = runtime.get('--kv-cache-dtype')
    if kv_dtype not in {'fp8','fp8_e4m3','fp8_e4m3fn'}:
        raise ValueError('Verified FP8 serving configuration required for this cache profile')
    if verify_server and ('--enable-prefix-caching' not in runtime or '--no-disable-hybrid-kv-cache-manager' not in runtime):
        raise ValueError('Enable prefix caching and the hybrid KV cache manager')
    # vLLM gated_delta_net_state_shape: convolution history is kernel_size-1;
    # recurrent state is value_heads * value_dim * key_dim (float32 here).
    if text.get('mamba_ssm_dtype') != 'float32' or text.get('dtype') != 'bfloat16':
        raise ValueError('Unexpected recurrent/convolution state dtype')
    per_token=text['num_key_value_heads']*text['head_dim']*2  # FP8 K+V
    recurrent=text['linear_num_value_heads']*text['linear_value_head_dim']*text['linear_key_head_dim']*4
    convolution=(text['linear_conv_kernel_dim']-1)*(2*text['linear_num_key_heads']*text['linear_key_head_dim']+text['linear_num_value_heads']*text['linear_value_head_dim'])*2
    aligned=math.ceil((recurrent+convolution)/per_token/32)*32
    block=int(os.getenv('VLLM_CACHE_BLOCK_TOKENS',str(aligned)))
    if block!=aligned: raise ValueError('Cache block override differs from resolved single-GPU hybrid geometry')
    if verify_server and not runtime.get('lmcache_separate_object_groups'):
        raise ValueError('LMCache --separate-object-groups is required to preserve hybrid recurrent state')
    if verify_server and runtime.get('lmcache_chunk_size') != str(block):
        raise ValueError('LMCache chunk size must match the resolved hybrid block size')
    if verify_server and int(runtime.get('--max-num-batched-tokens','0'))<block:
        raise ValueError('Prefill scheduler cannot advance one recurrent-state block')
    return {'model':MODEL,'model_config':str(path),'model_config_sha256':digest(raw),
            'tokenizer_files_sha256':tokenizer_files,
            'linear_layers':24,'full_attention_layers':8,
            'max_model_len':int(served['max_model_len']),
            'tokenizer_path':str(path.parent),'cache_block_tokens':block,
            'cache_block_origin':('single-GPU state geometry; LMCache process chunk size verified' if verify_server
                                  else 'offline FP8 single-GPU geometry; runtime block size unverified'),
            'serving_configuration_verified':verify_server,
            'kv_cache_dtype':kv_dtype,'attention_kv_bytes_per_token_per_layer':per_token,
            'recurrent_state_bytes_per_layer':recurrent,'convolution_state_bytes_per_layer':convolution,
            'runtime_cache_configuration':{k:v for k,v in runtime.items() if k!='--chat-template'},
            'recurrent_state_dtype':text.get('mamba_ssm_dtype','unspecified'),
            'weight_bits':4,'prefix_cache_note':'full-attention KV and linear recurrent state are managed by vLLM; no manual padding'}


def serving_configuration():
    """Read only selected non-secret flags; never expose process environments."""
    selected={}; proc=Path('/proc')
    for process in proc.iterdir():
        if not process.name.isdigit(): continue
        try: args=(process/'cmdline').read_bytes().decode(errors='replace').rstrip('\0').split('\0')
        except (OSError,PermissionError): continue
        if 'serve' in args and any(Path(arg).name=='vllm' for arg in args):
            flags=['--kv-cache-dtype','--max-model-len','--max-num-batched-tokens','--max-num-seqs','--tensor-parallel-size','--chat-template']
            for flag in flags:
                if flag in args: selected[flag]=args[args.index(flag)+1]
            for flag in ['--enable-prefix-caching','--no-disable-hybrid-kv-cache-manager','--enable-chunked-prefill']:
                if flag in args: selected[flag]=True
        if 'server' in args and any(Path(arg).name=='lmcache' for arg in args) and '--chunk-size' in args:
            selected['lmcache_chunk_size']=args[args.index('--chunk-size')+1]
            selected['lmcache_separate_object_groups']='--separate-object-groups' in args and '--no-separate-object-groups' not in args
    if int(selected.get('--tensor-parallel-size','1'))!=1:
        raise ValueError('This measured cache profile supports tensor parallel size 1')
    return selected



def balanced_cards(cards):
    """Round-robin sources to avoid a large repository starving other platforms."""
    queues={}
    for card in cards: queues.setdefault(card['repo'],[]).append(card)
    for queue in queues.values():
        queue.sort(key=lambda c:(-reference_priority(c),c['path'],c['line_start'],c['id']))
    result=[]
    for index in range(max(map(len,queues.values()),default=0)):
        for repo in sorted(queues):
            if index<len(queues[repo]): result.append(queues[repo][index])
    return result


def reference_priority(card):
    return 100 if card["path"] == "scripts/prefix/sempai-authoring-reference.md" else 50



def task_platforms(task):
    return {"brassclaw"}


def pipeline(refresh=False):
    sync(refresh)
    collect()
    for lane in ("ACTION", "REASONING", "THINKING"):
        distill_lane(lane)
    reduce_all()
    build_context(offline=True)


def cli_main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--mode",choices=["sync","collect","web","atoms","triage","distill","reduce","context","run","status","doctor","plan","reset","all"],default="plan")
    ap.add_argument("--refresh",action="store_true")
    ap.add_argument("--task",default=None)
    ap.add_argument("--context-tokens",type=int,default=CONTEXT_TARGET_TOKENS)
    ap.add_argument("--offline",action="store_true",
                    help="Context only: compile from ORNITH_MODEL_CONFIG without contacting or inspecting a server; cache profile remains unverified")
    args=ap.parse_args()
    if args.offline and args.mode!='context':
        ap.error('--offline is supported only with --mode context')
    try:
        if args.mode in {"all","triage","distill","reduce","context"}:
            ensure_pipeline_version()
        if args.mode=="sync": sync(args.refresh)
        elif args.mode=="collect": sync(args.refresh); collect()
        elif args.mode=="atoms": atoms_build()
        elif args.mode=="web": fetch_web_documents()
        elif args.mode=="triage": triage()
        elif args.mode=="distill":
            for lane in ("ACTION","REASONING","THINKING"):
                distill_lane(lane)
        elif args.mode=="reduce": reduce_all()
        elif args.mode=="context": build_context(args.context_tokens,offline=args.offline)
        elif args.mode=="run": init_client(); print(run_task(args.task or input("Task: ")))
        elif args.mode=="status": status()
        elif args.mode=="doctor": doctor()
        elif args.mode=="plan": plan()
        elif args.mode=="reset": reset()
        elif args.mode=="all": pipeline(args.refresh)
        log("Done")
    except KeyboardInterrupt:
        log("Stopped cleanly. Completed SQLite checkpoints were preserved.")
        sys.exit(130)
    except Exception as exc:
        log(f"FAILED: {type(exc).__name__}: {exc}")
        try:
            BASE.mkdir(parents=True,exist_ok=True)
            atomic_write(FAILED,json.dumps({"error":f"{type(exc).__name__}: {exc}","time":time.time()},indent=2))
        except Exception:
            pass
        raise


def main():
    preview = argparse.ArgumentParser(add_help=False)
    preview.add_argument('--mode', default='plan')
    requested, _ = preview.parse_known_args()
    if requested.mode == 'plan' or '--help' in sys.argv:
        cli_main()
        return
    import fcntl
    BASE.mkdir(parents=True,exist_ok=True)
    with (BASE/"pipeline.lock").open("a") as lock:
        try:
            fcntl.flock(lock,fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError("Another pipeline command is running against this cache")
        cli_main()


if __name__=="__main__":
    main()
