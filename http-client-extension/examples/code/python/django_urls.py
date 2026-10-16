# Django URLconf.
# Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
from django.urls import path, re_path

from . import views

urlpatterns = [
    path("articles/<int:year>/", views.year_archive),
    path("articles/<int:year>/<int:month>/", views.month_archive),
    path("authors/<slug:name>/", views.author),
    re_path(r"^legacy/(?P<pk>[0-9]+)/$", views.legacy),
]
