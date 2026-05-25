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
            **kwargs
    ):
        super().__init__(
            text="Loading...",
            **kwargs
        )
        self.url=url
        self.method=method
        self.headers=headers or {}
        self.format_response=format_response
        self.loading=False
        # Trigger first fetch immediately; periodic updates still follow update_interval.
        self.update(0)
    
    def fetch_data(self):
        try:
            response = requests.get(
                self.url,
                headers=self.headers,
                timeout=5
            )

            text =response.text
            if self.format_response:
                text=self.format_response(text)
            self.text=text
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
