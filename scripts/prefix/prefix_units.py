"""Effect-free whole-unit segmentation shared with preloaded compiler code."""

def source_units_core(text, suffix):
    """Complete documentation sections; executable files remain whole.

    Literal parent preambles travel with children. If these are too large, the
    complete card is omitted from the bounded prefix rather than trimming its
    qualifications. RST literal blocks are indented and cannot become headings.
    """
    import re
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

