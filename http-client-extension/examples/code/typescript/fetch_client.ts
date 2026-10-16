// fetch (any runtime).
// Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
export async function listOrders() {
  return fetch("/api/orders");
}

export async function createOrder(order: Order) {
  return fetch("/api/orders", {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Tenant": "demo" },
    body: JSON.stringify({ customer: order.customer, total: order.total }),
  });
}

export async function removeOrder(orderId: string) {
  return fetch(`https://api.example.com/orders/${orderId}`, { method: "DELETE" });
}

interface Order {
  customer: string;
  total: number;
}
