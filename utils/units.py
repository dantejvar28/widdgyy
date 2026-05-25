def resolve_unit(value,total_size):

    if isinstance(value, int):
        return value
    if isinstance(value, float):
        return int(value)
    
    if isinstance(value,str):
        value=value.strip()
        if value.endswith("%"):
            percent=float(
                value.replace("%","")
            )

            return int(
                total_size*(percent/100)
            )
        
        if value.endswith("px"):
            return int(
                value.replace("px","")
            )
    return None