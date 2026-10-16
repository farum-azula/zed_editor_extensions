# requests / httpx / aiohttp.
# Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
import httpx
import requests

def get_user(user_id: str):
    return requests.get(f"https://api.example.com/users/{user_id}", params={"expand": "profile"})


def create_user(name: str, email: str):
    return requests.post(
        "https://api.example.com/users",
        json={"name": name, "email": email},
        headers={"X-Request-Id": "demo"},
    )


async def delete_user(user_id: str):
    async with httpx.AsyncClient() as client:
        return await client.delete(f"https://api.example.com/users/{user_id}")
