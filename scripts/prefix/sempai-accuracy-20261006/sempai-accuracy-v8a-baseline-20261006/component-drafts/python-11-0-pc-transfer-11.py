def classify_rank(inputs):
    if 'rank' not in inputs:
        result = {'kind': 'missing', 'rank': None}
    else:
        rank = inputs['rank']
        if rank is None:
            result = {'kind': 'null', 'rank': None}
        elif type(rank) is int and not isinstance(rank, bool) and 7 <= rank <= 16:
            result = {'kind': 'integer', 'rank': rank}
        else:
            result = {'kind': 'invalid', 'rank': None}
    return result
result = classify_rank(inputs.get('rank'))