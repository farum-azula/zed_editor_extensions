// Hono.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
import { Hono } from "hono";

const app = new Hono();

app.get("/posts/:id", (c) => {
  const expand = c.req.query("expand");
  return c.json({ id: c.req.param("id"), expand });
});

app.post("/posts", async (c) => {
  const { title, body } = await c.req.json();
  const tenant = c.req.header("X-Tenant");
  return c.json({ title, body, tenant }, 201);
});

app.delete("/posts/:id", (c) => c.body(null, 204));

export default app;
