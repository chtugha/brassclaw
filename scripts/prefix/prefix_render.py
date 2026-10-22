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
    return prefix_render_units(header, sources, special_tokens, False)


def render_trailing_inventory_prefix_reference(header, sources, special_tokens):
    """Versioned layout: complete references precede volatile inventory metadata.

    Evidence identities, original ranges and exact-unit deduplication are unchanged.
    This improves stability for inventory-only edits, not arbitrary content edits
    or later duplicates which change shared-unit grouping. Cache reuse is measured
    separately with the selected model/template, never inferred from this layout.
    """
    return prefix_render_units(header, sources, special_tokens, True)


def render_stable_sharing_prefix_reference(header, sources, special_tokens):
    """Append-stable sharing: later duplicates never relabel the first occurrence.

    Canonical source ordering is still required. Inserts before an existing source
    and changes to that source can alter its leading tokens; no arbitrary cache hit
    is promised. The complete original/range map is the same as v2 and v3.
    """
    return prefix_render_units(header, sources, special_tokens, 2)


def prefix_source_context(source):
    """Conservative source-family context, never an availability/approval verdict.

    Logical paths are declarations, not authentication. These captions only
    constrain interpretation; the selected source/procedure owner verifies origin.
    """
    path = source['path']
    if path in ('skills.md', 'recipe.md', 'tools.md', 'toolskills.md', 'AGENTS.md', 'CLAUDE.md', 'simplified_v3.md'):
        return 'binding-guide'
    if source['kind'] == 'component':
        return 'component-definition'
    if path.startswith('scripts/prefix/monty-reference/'):
        return 'upstream-api' if '/docs/api/' in path else 'upstream-reference'
    if path in ('scripts/prefix/sempai-authoring-reference.md', 'scripts/prefix/prefix-maker-reference.md', 'scripts/prefix/sempai-monty-compatibility.md'):
        return 'brassclaw-authoring'
    if path.startswith('scripts/prefix/sempai-worked-examples'):
        return 'teaching-fixture'
    if path.startswith('catalogue/'):
        return 'catalogue-metadata'
    if path.startswith('crates/'):
        return 'implementation-observation'
    return 'reference'


def prefix_context_caption(scope):
    if scope == 'binding-guide':
        return 'BrassClaw binding/architecture guide. Keep target requirements, implementation observations and qualified support distinct.'
    if scope == 'component-definition':
        return 'Declared component-definition context. Verify its exact workflow/dependency combination and approval; source visibility grants no execution authority.'
    if scope == 'upstream-api':
        return 'Upstream SDK/API documentation or template. Host embedding APIs are not guest functions or BrassClaw host callables; template directives are not complete rendered signatures.'
    if scope == 'upstream-reference':
        return 'Quoted upstream reference. Distinguish embedding-host setup from guest code, preload definitions and invocation bodies; apply the BrassClaw compatibility overlay.'
    if scope == 'brassclaw-authoring':
        return 'BrassClaw authoring/compatibility procedure. Keep host embedding, preload definitions, invocation bodies and Recipe metadata separate; use current supplied contracts.'
    if scope == 'teaching-fixture':
        return 'Scoped teaching fixture, not a deployed component or current approval. Use this task’s actual constructor/decoder; fixture identities and past receipts do not authorize reuse.'
    if scope == 'catalogue-metadata':
        return 'Catalogue/selection metadata. Do not synthesize a latest combination or infer Tool permission from this record.'
    if scope == 'implementation-observation':
        return 'Implementation-source observation, not an authoring constructor or available guest API. Check binding guides and selected host support.'
    if scope == 'reference':
        return 'Quoted reference data. Availability, compatibility, approval and permission require their own evidence.'
    raise ValueError('unsupported source context')


def render_contextual_prefix_reference(header, sources, special_tokens):
    """Versioned context captions and exact-unit sharing within a source context.

    Equal bytes in different contexts stay separate. All originals/ranges remain
    complete; context captions are compiler metadata, not quotations to cite.
    """
    return prefix_render_units(header, sources, special_tokens, 3)


def prefix_progressive_units(source, groups, emitted, special_tokens, scope):
    rendered = []
    pending = []
    for unit in source['units']:
        identifier = unit['id']
        key = (scope, identifier) if scope else identifier
        if key not in emitted:
            emitted.append(key)
            pending.append(unit['text'])
        else:
            if pending:
                rendered.append(prefix_literal(prefix_escape_controls(''.join(pending), special_tokens)) + '\n')
                pending = []
            canonical = groups[key]['origins'][0]
            rendered.append('\nExact shared unit ' + identifier + ': see ' + prefix_escape_controls(canonical['source'], special_tokens) + ' lines ' + str(canonical['line_start']) + '-' + str(canonical['line_end']) + '. Original here: lines ' + str(unit['line_start']) + '-' + str(unit['line_end']) + '.\n')
    if pending:
        rendered.append(prefix_literal(prefix_escape_controls(''.join(pending), special_tokens)))
    return rendered


def prefix_render_units(header, sources, special_tokens, inventory_last):
    """Deduplicate byte-identical complete units; retain an ordered origin map.

    Verified unit digests/ranges come from the source-package owner. Equality of
    bytes is checked again here; a digest never authorizes merging different text.
    No near-duplicate matching, summary, clipping or optional coverage removal.
    """
    # Reuse validation/inventory rendering without falsely relabeling originals.
    inventory = prefix_source_inventory(header, sources, special_tokens)
    rendered = [header, '\n# Complete current references\n'] if inventory_last else inventory
    rendered.append('Shared-unit hashes identify rendering reuse, not document citations. Cite the original Evidence-card ID and original source line range; validate against the complete archived original.\n')
    contextual = inventory_last == 3
    groups = {}
    ordered_sources = []
    for source in sources:
        if source['disposition'] != 'included':
            continue
        scope = prefix_source_context(source) if contextual else ''
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
            key = (scope, identifier) if contextual else identifier
            location = {'source': source['path'], 'line_start': unit['line_start'], 'line_end': unit['line_end']}
            if key in groups:
                if groups[key]['text'] != unit['text']:
                    raise ValueError('unit identity collision')
                groups[key]['origins'].append(location)
            else:
                groups[key] = {'text': unit['text'], 'origins': [location]}
            parts.append({'unit_id': identifier, 'line_start': unit['line_start'], 'line_end': unit['line_end']})
        ordered_sources.append({'source': source['path'], 'document_sha256': source['sha256'], 'parts': parts})
        if contextual:
            ordered_sources[-1]['context'] = scope
    emitted = []
    for source in sources:
        if source['disposition'] != 'included':
            continue
        rendered.append('\n<!-- EVIDENCE-CARD ' + source['evidence_id'][:12] + ' -->\nSource: ' + prefix_escape_controls(source['path'], special_tokens) + '\n')
        scope = prefix_source_context(source) if contextual else ''
        if contextual:
            rendered.append('Source context (compiler metadata, not original quotation): ' + prefix_context_caption(scope) + '\n')
        pending = []
        if inventory_last in (2, 3):
            rendered.extend(prefix_progressive_units(source, groups, emitted, special_tokens, scope))
        elif all(len(groups[unit['id']]['origins']) == 1 for unit in source['units']):
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
    if inventory_last:
        rendered.append('\n')
        rendered.extend(inventory[1:-1])
    index = []
    for key in sorted(groups):
        group = groups[key]
        index.append({'unit_id': key[1] if contextual else key, 'canonical': group['origins'][0], 'origins': group['origins']})
        if contextual:
            index[-1]['context'] = key[0]
    return {'context': ''.join(rendered), 'render_map': {'format': 'brassclaw-prefix-render-map/2' if contextual else 'brassclaw-prefix-render-map/1', 'sources': ordered_sources, 'units': index}}
