# vLLM Prefix-Cache Tuning

## Intent

Enable prefix caching on the vLLM inference server at `192.168.10.223:8000`
so repeated system-prompt + context prefixes hit the KV cache rather than
re-computing them on every turn. With `Qwen2.5-7B-Instruct-AWQ` and a 16K
context window this can reduce TTFT by 40–70% on warm requests.

## Launch Flags

Add these flags to the vLLM service unit (or drop-in) on `192.168.10.223`:

```
--enable-prefix-caching
--gpu-memory-utilization 0.90
--max-model-len 16384
--kv-cache-dtype auto
```

## Systemd Drop-in

Create `/etc/systemd/system/vllm.service.d/prefix-cache.conf`:

```ini
[Service]
# Prefix-cache tuning for Qwen2.5-7B-Instruct-AWQ (16K context)
ExecStart=
ExecStart=/usr/bin/python3 -m vllm.entrypoints.openai.api_server \
    --model Qwen/Qwen2.5-7B-Instruct-AWQ \
    --port 8000 \
    --enable-prefix-caching \
    --gpu-memory-utilization 0.90 \
    --max-model-len 16384 \
    --kv-cache-dtype auto
```

Apply with:

```bash
systemctl daemon-reload
systemctl restart vllm
```

## Verification

After a warm request (two turns on the same thread), check hit rate:

```bash
curl -s http://192.168.10.223:8000/metrics | grep prefix_cache_hit
```

Expected output (after warm requests):
```
vllm:cpu_prefix_cache_hit_rate{...} 0.xx
vllm:gpu_prefix_cache_hit_rate{...} 0.xx
```

A non-zero `gpu_prefix_cache_hit_rate` confirms prefix caching is active.

## Notes

- `--gpu-memory-utilization 0.90` leaves ~10% headroom on the GPU to avoid
  OOM during long generations.
- `--kv-cache-dtype auto` selects the best dtype for the GPU (fp8 on H100/A100,
  fp16 elsewhere). Override with `fp16` if quantization issues appear.
- `--max-model-len 16384` matches the model's advertised 16K context window.
  Do NOT exceed this — the model weights only encode positional embeddings
  up to this length.
