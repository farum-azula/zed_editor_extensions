// Express.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
import express from "express";

const app = express();
app.use(express.json());

app.get("/users/:id", (req, res) => {
  const fields = req.query.fields;
  res.json({ id: req.params.id, fields });
});

app.post("/users", (req, res) => {
  const { name, email, password } = req.body;
  const tenant = req.header("X-Tenant");
  res.status(201).json({ name, email, tenant });
});

app.delete("/users/:id", (req, res) => {
  if (!req.headers.authorization) return res.sendStatus(401);
  res.sendStatus(204);
});

app.listen(8080);
