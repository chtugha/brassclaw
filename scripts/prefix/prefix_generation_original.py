"""Project one whole archived source from the existing generation-read result.

The caller must supply the real immutable storage result. This pure projection
does not authenticate that custody, hash content, fetch, approve or publish health.
"""


def original_nonempty(value):
    if not isinstance(value, str) or not value.strip():
        raise ValueError('nonempty original-source metadata required')
    return value


def original_digest(value):
    if not isinstance(value, str) or len(value) != 64 or any(ch not in '0123456789abcdef' for ch in value):
        raise ValueError('exact original-source digest required')
    return value


def original_artifact(package, name):
    import base64
    artifacts = package.get('artifacts')
    if not isinstance(artifacts, dict) or name not in artifacts:
        raise ValueError('complete archived original artifact required')
    artifact = artifacts[name]
    if not isinstance(artifact, dict) or sorted(artifact) != ['data', 'encoding'] or artifact['encoding'] != 'base64':
        raise ValueError('supported original artifact encoding required')
    data = artifact['data']
    if not isinstance(data, str):
        raise ValueError('exact archived base64 bytes required')
    decoded = base64.b64decode(data, validate=True)
    if base64.b64encode(decoded).decode('ascii') != data:
        raise ValueError('canonical archived base64 bytes required')
    return decoded.decode('utf-8')


def project_prefix_generation_source(inputs):
    import json
    if not isinstance(inputs, dict) or sorted(inputs) != ['detail', 'generation', 'profile', 'reference']:
        raise ValueError('exact original-source projection inputs required')
    profile = original_nonempty(inputs['profile'])
    generation = original_digest(inputs['generation'])
    reference = original_nonempty(inputs['reference'])
    detail = inputs['detail']
    if not isinstance(detail, dict) or not isinstance(detail.get('summary'), dict):
        raise ValueError('immutable generation-read result required')
    summary = detail['summary']
    if summary.get('profile') != profile or summary.get('generation') != generation:
        raise ValueError('original belongs to another profile or generation')
    if summary.get('kind') != 'compiler-package':
        raise ValueError('legacy generation has no recoverable original source index')
    qualification = original_nonempty(summary.get('qualification'))
    package = json.loads(original_nonempty(detail.get('generation_bytes')))
    if not isinstance(package, dict) or package.get('format') != 'brassclaw-prefix-generation-package/1' or package.get('profile') != profile:
        raise ValueError('unsupported archived original package')
    index = json.loads(original_artifact(package, 'sources.json'))
    if not isinstance(index, dict) or index.get('format') not in ['brassclaw-prefix-generation-sources/1', 'brassclaw-prefix-source-package/1'] or not isinstance(index.get('sources'), list):
        raise ValueError('complete archived source index required')
    matches = [row for row in index['sources'] if isinstance(row, dict) and row.get('path') == reference]
    if len(matches) != 1:
        raise ValueError('exactly one archived source reference required')
    row = matches[0]
    digest = original_digest(row.get('sha256'))
    artifact = 'raw/' + digest
    # Historical artifact names belong to the input package, not this generation.
    # Both native-supported index formats archive originals under raw/<sha256>.
    if index['format'] == 'brassclaw-prefix-generation-sources/1' and row.get('artifact') != artifact:
        raise ValueError('source index and raw artifact identity differ')
    return {'profile': profile, 'generation': generation, 'reference': reference,
            'original_sha256': digest, 'original_text': original_artifact(package, artifact),
            'disposition': original_nonempty(row.get('disposition')), 'reason': original_nonempty(row.get('reason')),
            'generation_qualification': qualification,
            'qualification': 'archived-original-projection-only; native read/digest custody required; no availability, knowledge approval or activation'}
