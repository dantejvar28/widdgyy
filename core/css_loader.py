import re 

class CSSLoader:
    def __init__(self, path):
        self.path =path 

        self.styles={}

    def load(self):
        with open (self.path) as f:
            css=f.read()

        blocks = re.findall(
            r'\.(.*?)\s*\{(.*?)\}',
            css,
            re.DOTALL
        )

        for class_name, body in blocks:
            class_name=class_name.strip()
            props={}
            lines=body.split(";")
            for line in lines: 
                line=line.strip()
                if not line:
                    continue

                if ":" not in line:
                    continue
                key, value = line.split(":",1)
                normalized_key = key.strip().replace("-", "_")
                props[normalized_key]=value.strip()
            self.styles[class_name]=props
    
    def get_style(self,class_name):
        return self.styles.get(class_name,{})