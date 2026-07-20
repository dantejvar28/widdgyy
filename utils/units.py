# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later

def resolve_unit(value,total_size):

    if value is None:
        return None
    if isinstance(value, bool):
        return None

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