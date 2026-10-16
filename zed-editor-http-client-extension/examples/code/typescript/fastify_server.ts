// Fastify.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
import Fastify from "fastify";

interface CreateUser {
  name: string;
  email: string;
  admin: boolean;
}

const fastify = Fastify();

fastify.post<{ Body: CreateUser }>("/users", async (request) => request.body);

fastify.get("/users/:id", async (request) => {
  const trace = request.headers["x-trace-id"];
  return { trace };
});

fastify.route({
  method: "PUT",
  url: "/users/:id",
  handler: async () => ({}),
});

fastify.listen({ port: 8080 });
