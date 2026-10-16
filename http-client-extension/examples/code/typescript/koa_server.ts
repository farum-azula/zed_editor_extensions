// Koa router.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
import Router from "@koa/router";

const router = new Router();

router.get("/articles/:id", (ctx) => {
  const draft = ctx.query.draft;
  ctx.body = { id: ctx.params.id, draft };
});

router.post("/articles", (ctx) => {
  const { title, text } = ctx.request.body;
  const tenant = ctx.get("X-Tenant");
  ctx.body = { title, text, tenant };
});

router.put("/articles/:id", (ctx) => {
  ctx.status = 204;
});

export default router;
