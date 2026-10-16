// Refit.
// Click "🌐 Generate .http request" above a method, or press cmd+. on the line.
using Refit;

namespace Example.Api;

public record Issue(string Title, string Body, bool Open);

public interface IGitHubApi
{
    [Get("/users/{user}")]
    Task<User> GetUser(string user);

    [Post("/repos/{owner}/issues")]
    Task<Issue> CreateIssue(string owner, [Header("X-Request-Id")] string rid, [Body] Issue issue);

    [Delete("/repos/{owner}/{repo}")]
    Task DeleteRepo(string owner, string repo);
}
