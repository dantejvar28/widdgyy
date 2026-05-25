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
            **kwargs
    ):
        super().__init__(
            text="Loading...",
            **kwargs
        )
        self.url=url
        self.template=template
        self.fields=fields or {}
        self.method=method
        self.headers=headers or {}
        
        self.loading=False
        # Trigger first fetch immediately; periodic updates still follow update_interval.
        self.update(0)
    
    def extract_json_path(self,data,path):
        keys=path.split(".")
        value=data
        for key in keys:
            if key.isdigit():
                value=value[int(key)]
            else: 
                value=value[key]
        return value


    def fetch_data(self):
        try:
            response = requests.get(
                self.url,
                headers=self.headers,
                timeout=5
            )

            data = response.json()
            values={}
            for name, path in self.fields.items():
                values[name]=self.extract_json_path(data,path)
            self.text=self.template.format(**values)
            
        except Exception as e:
            self.text=f"API Error"
        
        self.loading = False 

    def update(self,data):
        if self.loading:
            return
        
        self.loading = True
        threading.Thread(
            target=self.fetch_data,
            daemon=True
        ).start()
