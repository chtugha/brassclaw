"""Effect-free reference rendering shared by offline and preloadable execution.

No imports, file/network access, hashing, global mutable state or implicit host
callables. The caller supplies already verified, ordered source/evidence data.
Rendering establishes no source approval, token capacity or activation status.
"""

def prefix_literal(text):
    longest = 0
    position = 0
    while True:
        start = text.find('`', position)
        if start == -1:
            break
        end = start + 1
        while end < len(text) and text[end] == '`':
            end += 1
        if end - start > longest:
            longest = end - start
        position = end
    length = longest + 1
    if length < 3:
        length = 3
    fence = '`' * length
    return fence + 'text\n' + text + ('' if text.endswith('\n') else '\n') + fence


def prefix_escape_controls(text, special_tokens):
    # Search literal spans in the native string implementation. Only candidate
    # control markers enter Python; ordinary code/YAML bytes are copied whole.
    result = []
    literal_start = 0
    position = 0
    while True:
        marker = text.find('<', position)
        if marker == -1:
            break
        end = -1
        if text[marker:marker + 2] == '<|':
            closing = text.find('|>', marker + 2)
            if closing != -1 and closing - marker <= 130 and '\n' not in text[marker:closing] and '\r' not in text[marker:closing]:
                end = closing + 2
        elif text[marker:marker + 4] == '<!--':
            closing = text.find('-->', marker + 4)
            if closing != -1 and '\n' not in text[marker:closing] and '\r' not in text[marker:closing]:
                body = text[marker + 4:closing].strip()
                if body.startswith('EVIDENCE-CARD') or body.startswith('END-EVIDENCE-CARD'):
                    end = closing + 3
        if end != -1:
            result.append(text[literal_start:marker])
            result.append(text[marker:end].replace('<', '\\u003c').replace('>', '\\u003e'))
            literal_start = end
            position = end
        else:
            position = marker + 1
    result.append(text[literal_start:])
    escaped = ''.join(result)
    for token in sorted(special_tokens, key=len, reverse=True):
        if token:
            replacement = ''.join('\\u%04x' % ord(character) for character in token)
            escaped = escaped.replace(token, replacement)
    return escaped


def prefix_source_inventory(header, sources, special_tokens):
    if not header.strip():
        raise ValueError('nonempty trusted profile frame required')
    rendered = [header, '# Source inventory\n']
    seen = []
    for source in sources:
        path = source['path']
        if source['disposition'] not in ('included', 'historical', 'development-only', 'superseded'):
            raise ValueError('unsupported source disposition')
        if path in seen:
            raise ValueError('duplicate logical source')
        seen.append(path)
        rendered.append(prefix_escape_controls('- ' + path + ' | ' + source['disposition'] + ' | ' + source['reason'] + '\n', special_tokens))
    rendered.append('\n# Complete current references\n')
    return rendered


def render_prefix_reference(header, sources, special_tokens):
    """One literal rendering per included original; excluded originals are indexed.

    Sources must be pinned in canonical order by the caller. Each included source
    supplies its complete text and independently computed evidence identifier.
    All temporary arrays stay local to this invocation.
    """
    rendered = prefix_source_inventory(header, sources, special_tokens)
    compact_ids = []
    for source in sources:
        if source['disposition'] != 'included':
            continue
        compact = source['evidence_id'][:12]
        if compact in compact_ids:
            raise ValueError('compact evidence identifiers collided')
        compact_ids.append(compact)
        text = source['text']
        if not text.strip():
            raise ValueError('empty included reference')
        rendered.append('\n<!-- EVIDENCE-CARD ' + compact + ' -->\nSource: ' + prefix_escape_controls(source['path'], special_tokens) + '\n' + prefix_literal(prefix_escape_controls(text, special_tokens)) + '\n<!-- END-EVIDENCE-CARD -->\n')
    return ''.join(rendered)


def render_deduplicated_prefix_reference(header, sources, special_tokens):
    """Deduplicate byte-identical complete units; retain an ordered origin map.

    Verified unit digests/ranges come from the source-package owner. Equality of
    bytes is checked again here; a digest never authorizes merging different text.
    No near-duplicate matching, summary, clipping or optional coverage removal.
    """
    # Reuse validation/inventory rendering without falsely relabeling originals.
    rendered = prefix_source_inventory(header, sources, special_tokens)
    rendered.append('Shared-unit hashes identify rendering reuse, not document citations. Cite the original Evidence-card ID and original source line range; validate against the complete archived original.\n')
    groups = {}
    ordered_sources = []
    for source in sources:
        if source['disposition'] != 'included':
            continue
        units = source['units']
        if ''.join(unit['text'] for unit in units) != source['text']:
            raise ValueError('unit partition does not preserve the complete original')
        expected_start = 1
        parts = []
        for unit in units:
            if unit['line_start'] != expected_start or unit['line_end'] != expected_start + len(unit['text'].splitlines()) - 1:
                raise ValueError('unit range differs from original lines')
            expected_start = unit['line_end'] + 1
            identifier = unit['id']
            location = {'source': source['path'], 'line_start': unit['line_start'], 'line_end': unit['line_end']}
            if identifier in groups:
                if groups[identifier]['text'] != unit['text']:
                    raise ValueError('unit identity collision')
                groups[identifier]['origins'].append(location)
            else:
                groups[identifier] = {'text': unit['text'], 'origins': [location]}
            parts.append({'unit_id': identifier, 'line_start': unit['line_start'], 'line_end': unit['line_end']})
        ordered_sources.append({'source': source['path'], 'document_sha256': source['sha256'], 'parts': parts})
    emitted = []
    for source in sources:
        if source['disposition'] != 'included':
            continue
        rendered.append('\n<!-- EVIDENCE-CARD ' + source['evidence_id'][:12] + ' -->\nSource: ' + prefix_escape_controls(source['path'], special_tokens) + '\n')
        pending = []
        if all(len(groups[unit['id']]['origins']) == 1 for unit in source['units']):
            rendered.append(prefix_literal(prefix_escape_controls(source['text'], special_tokens)))
        else:
            for unit in source['units']:
                identifier = unit['id']
                group = groups[identifier]
                if len(group['origins']) == 1:
                    pending.append(unit['text'])
                    continue
                if pending:
                    rendered.append(prefix_literal(prefix_escape_controls(''.join(pending), special_tokens)) + '\n')
                    pending = []
                canonical = group['origins'][0]
                if identifier in emitted:
                    rendered.append('\nExact shared unit ' + identifier + ': see ' + prefix_escape_controls(canonical['source'], special_tokens) + ' lines ' + str(canonical['line_start']) + '-' + str(canonical['line_end']) + '. Original here: lines ' + str(unit['line_start']) + '-' + str(unit['line_end']) + '.\n')
                else:
                    emitted.append(identifier)
                    rendered.append('\nExact shared unit ' + identifier + ' (complete original):\n' + prefix_literal(prefix_escape_controls(unit['text'], special_tokens)) + '\n')
        if pending:
            rendered.append(prefix_literal(prefix_escape_controls(''.join(pending), special_tokens)))
            pending = []
        rendered.append('\n<!-- END-EVIDENCE-CARD -->\n')
    index = []
    for identifier in sorted(groups):
        group = groups[identifier]
        index.append({'unit_id': identifier, 'canonical': group['origins'][0], 'origins': group['origins']})
    return {'context': ''.join(rendered), 'render_map': {'format': 'brassclaw-prefix-render-map/1', 'sources': ordered_sources, 'units': index}}
