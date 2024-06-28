import os
from openai import OpenAI

client = OpenAI(
    # This is the default and should be replaced with felafax API key
    api_key=os.environ.get("OPENAI_API_KEY"),
    # api_key=os.environ.get("FELAFAX_API_KEY"),

    # default is https://api.openai.com/v1/
    # base url for testing locally
    # base_url="http://127.0.0.1:8000/v1/"
    # base url for production instance
    # base_url = "https://felafax-proxy.shuttleapp.rs/v1/"
    base_url = "https://felafax-proxy-sq5shdnepa-uc.a.run.app/v1/"

)

completion = client.chat.completions.create(
    messages=[
        {
            "role": "user",
            "content": "Say this is a test",
        }
    ],
    model="gpt-3.5-turbo",
)

print(completion.choices[0].message.content)

