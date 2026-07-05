import threading 
import requests

from stickers.text import TextSticker

class APITextSticker(TextSticker):
    def __init__(
            self,
            url,
            method="GET",
            headers=None,
            format_response=None,
            template="",
            fields=None,
            timeout=10,
            retries=1,
            **kwargs
    ):
        super().__init__(
            text="Loading...",
            **kwargs
        )
        self.url = url
        self.template = template
        self.fields = fields or {}
        self.method = method
        self.timeout = timeout
        self.retries = retries
        self.headers = {
            "Accept": "application/json",
            "User-Agent": "ProyectoTest/1.0",
            **(headers or {})
        }
        
        self.loading = False
        # Trigger first fetch immediately; periodic updates still follow update_interval.
        self.update(0)
    
    def extract_json_path(self, data, path):
        keys = path.split(".")
        value = data
        for key in keys:
            if key.isdigit():
                index = int(key)
                if not isinstance(value, list) or index >= len(value):
                    raise KeyError(path)
                value = value[index]
            else: 
                if not isinstance(value, dict) or key not in value:
                    raise KeyError(path)
                value = value[key]
        return value


    def fetch_data(self):
        try:
            for _ in range(max(1, self.retries + 1)):
                try:
                    response = requests.request(
                        self.method,
                        self.url,
                        headers=self.headers,
                        timeout=self.timeout
                    )
                    response.raise_for_status()

                    data = response.json()
                    values = {}
                    for name, path in self.fields.items():
                        values[name] = self.extract_json_path(data, path)
                    self.text = self.template.format(**values)
                    self.mark_dirty()
                    return
                except Exception:
                    pass

            self.text = "API timeout/error"
            self.mark_dirty()
        except Exception:
            self.text = "API timeout/error"
            self.mark_dirty()
        finally:
            self.loading = False

    def update(self,data):
        if self.loading:
            return
        
        self.loading = True
        threading.Thread(
            target=self.fetch_data,
            daemon=True
        ).start()
        return True
