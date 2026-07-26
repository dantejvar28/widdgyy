# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later

import re
import threading
from datetime import datetime

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
        self.format_response = format_response
        self.headers = {
            "Accept": "application/json",
            "User-Agent": "ProyectoTest/1.0",
            **(headers or {})
        }

        self.loading = False
        # Trigger first fetch immediately; periodic updates still follow update_interval.
        self.update(0)

    def _parse_field_expression(self, expression):
        path, *filters = [part.strip() for part in str(expression).split("|")]
        if not path:
            raise KeyError(expression)
        return path, [field_filter for field_filter in filters if field_filter]

    def extract_json_path(self, data, path):
        value = data
        tokens = re.findall(r"([^\.\[\]]+)|\[(\d+)\]", path)
        if not tokens:
            raise KeyError(path)

        for key, raw_index in tokens:
            if key and key.isdigit():
                index = int(key)
                if not isinstance(value, list) or index >= len(value):
                    raise KeyError(path)
                value = value[index]
                continue

            if key:
                if not isinstance(value, dict) or key not in value:
                    raise KeyError(path)
                value = value[key]
                continue

            index = int(raw_index)
            if not isinstance(value, list) or index >= len(value):
                raise KeyError(path)
            value = value[index]

        return value

    def apply_field_filter(self, value, filter_name):
        if filter_name == "weekday_short":
            if not isinstance(value, str):
                raise ValueError(f"weekday_short expects a date string, got {type(value).__name__}")
            normalized = value.replace("Z", "+00:00")
            try:
                date_value = datetime.fromisoformat(normalized)
            except ValueError:
                date_value = datetime.fromisoformat(f"{normalized}T00:00:00")
            return date_value.strftime("%a")

        raise ValueError(f"Unsupported field filter: {filter_name}")

    def resolve_field_value(self, data, expression):
        path, filters = self._parse_field_expression(expression)
        value = self.extract_json_path(data, path)
        for filter_name in filters:
            value = self.apply_field_filter(value, filter_name)
        return value

    def fetch_data(self):
        last_error = None
        try:
            for _ in range(max(1, self.retries + 1)):
                try:
                    print(f"Fetching data from {self.url}...")
                    response = requests.request(
                        self.method,
                        self.url,
                        headers=self.headers,
                        timeout=self.timeout
                    )
                    print(f"Response status code: {response.status_code}")
                    response.raise_for_status()

                    data = response.json()
                    values = {}
                    for name, path in self.fields.items():
                        values[name] = self.resolve_field_value(data, path)

                    self.text = self.template.format(**values)
                    self.mark_dirty()
                    return
                except requests.RequestException as exc:
                    last_error = exc
                    print(f"APITextSticker request failed: {exc}")
                except Exception as exc:
                    last_error = exc
                    print(f"APITextSticker parse failed: {exc}")
                    break

            if isinstance(last_error, requests.RequestException):
                self.text = "API timeout/error"
            else:
                self.text = "API parse error"
            self.mark_dirty()
        except Exception as exc:
            print(f"APITextSticker unexpected failure: {exc}")
            self.text = "API parse error"
            self.mark_dirty()
        finally:
            self.loading = False

    def update(self, data):
        if self.loading:
            return

        self.loading = True
        threading.Thread(
            target=self.fetch_data,
            daemon=True
        ).start()
        return True
