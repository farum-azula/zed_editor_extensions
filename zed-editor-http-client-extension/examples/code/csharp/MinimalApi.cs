// ASP.NET Core minimal APIs.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
var builder = WebApplication.CreateBuilder(args);
var app = builder.Build();

app.MapGet("/todos", () => new[] { new Todo("demo", false) });

app.MapGet("/todos/{id}", (int id) => id);

app.MapPost("/todos", (Todo todo) => todo);

app.MapDelete("/todos/{id}", (int id) => Results.NoContent());

app.Run();

public record Todo(string Title, bool Done);
