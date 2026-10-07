def check_channel_list(channels):
    if not isinstance(channels, list):
        return {'ok': False, 'channels': []}
    ok = True
    for item in channels:
        if isinstance(item, bool) or not isinstance(item, int) or not 0 <= item <= 255:
            ok = False
            break
    return {'ok': ok, 'channels': channels if ok else []}
result = check_channel_list(inputs.get('channels'))