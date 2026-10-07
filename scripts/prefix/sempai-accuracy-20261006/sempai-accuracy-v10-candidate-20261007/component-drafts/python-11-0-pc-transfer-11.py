rank = inputs.get('rank')
if 'rank' not in inputs:
    result = {'kind': 'missing', 'rank': None}
elif rank is None:
    result = {'kind': 'null', 'rank': None}
elif type(rank) is int and 7 <= rank <= 16:
    result = {'kind': 'integer', 'rank': rank}
else:
    result = {'kind': 'invalid', 'rank': None}