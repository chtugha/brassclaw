rank = inputs.get('rank')
accepted = type(rank) is int and not isinstance(rank, bool) and 1 <= rank <= 10
result = {'accepted': accepted, 'rank': rank if accepted else None}