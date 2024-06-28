# Felafax Proxy

## Overview
felafax-proxy is fast and lightweight proxy for LLMs written in Rust. It's designed to be low latency and scale well.

## Goals
* Translates all LLM APIs into OpenAI spec; enables adopting new LLMs easily.
* Low latency highly scalable; which is ideal for production usecases.
* Support hot-swapping of LLMs without needing code change.

## Usage
```py
import os
from openai import OpenAI

client = OpenAI(
    # Pass in felafax api key from portal
    api_key=os.environ.get("FELAFAX_API_KEY"),
    # Update the url to point to proxy
    base_url = "https://felafax-proxy.shuttleapp.rs/v1/"

)

completion = client.chat.completions.create(
    messages=[
        {
            "role": "user",
            "content": "Say this is a test",
        }
    ],
    # Set this to hot-swap to switch between LLMs using config
    # model="hot-swap",
    # Or pass the model name directly and we'll convert it to the right LLM
    model="gpt-3.5-turbo",
    # model="claude-3-5-sonnet-20240620"
    # model="jamba-instruct-preview"
)

print(completion.choices[0].message.content)
```

# Run
```sh
# docker build
docker build -t felafax-proxy .
# docker run
docker run -p 8080:8080 -v $(pwd)/firebase.json:/firebase.json felafax-proxy
```

# Supported Features

### Supported LLMs
- [x] OpenAI
- [x] Claude
- [x] Jamba

We support `/chat/completions` for each of these LLMs.

## TODO
- [ ] Adding `streaming` support
- [ ] Easy deployable config
- [ ] Request stats logging
- [ ] Routing logs to S3 or other storage connectors (Splunk)
- [ ] Monitoring & observability


