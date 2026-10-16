# FastAPI.
# Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
from fastapi import Depends, FastAPI, Header
from pydantic import BaseModel

app = FastAPI()


class ItemIn(BaseModel):
    name: str
    price: float
    in_stock: bool = True
    tags: list[str] = []


@app.get("/items/{item_id}")
def read_item(item_id: int, verbose: bool = False):
    return {"id": item_id}


@app.post("/items")
def create_item(item: ItemIn, x_tenant: str = Header(...), user=Depends(get_current_user)):
    return item


@app.patch("/items/{item_id}")
def update_item(item_id: int, item: ItemIn):
    return {"id": item_id, **item.dict()}


@app.delete("/items/{item_id}")
def delete_item(item_id: int):
    return None
