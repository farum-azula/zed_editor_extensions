// HttpClient.
// Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
using System.Net.Http;
using System.Net.Http.Json;

namespace Example.Services;

public record Todo(string Title, bool Done);

public class TodoService
{
    private readonly HttpClient _client;

    public TodoService(HttpClient client) => _client = client;

    public Task<Todo> GetAsync() =>
        _client.GetFromJsonAsync<Todo>("https://jsonplaceholder.typicode.com/todos/1");

    public Task<HttpResponseMessage> CreateAsync(Todo todo) =>
        _client.PostAsJsonAsync("/todos", todo);

    public Task<HttpResponseMessage> DeleteAsync(int id) =>
        _client.DeleteAsync($"/todos/{id}");
}
