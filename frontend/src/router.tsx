import { createBrowserRouter, Navigate, Outlet } from "react-router-dom";
import { Layout } from "@/components/layout/Layout";
import { useAppStore } from "@/lib/store";
import { HomePage } from "@/pages/HomePage";
import { BrowseLivePage } from "@/pages/BrowseLivePage";
import { SeriesPage } from "@/pages/SeriesPage";
import { FilmPage } from "@/pages/FilmPage";
import { WatchPage } from "@/pages/WatchPage";
import { LiveWatchPage } from "@/pages/LiveWatchPage";
import { CatalogPage } from "@/pages/CatalogPage";
import { CategoryPage } from "@/pages/CategoryPage";
import { SearchPage } from "@/pages/SearchPage";
import { WatchlistPage } from "@/pages/WatchlistPage";
import { LibraryPage } from "@/pages/LibraryPage";
import { FollowingPage } from "@/pages/FollowingPage";
import { ProfilePage } from "@/pages/ProfilePage";
import { SettingsPage } from "@/pages/SettingsPage";
import { StudioPage } from "@/pages/StudioPage";
import { AdHubPage } from "@/pages/AdHubPage";
import { AuthCallbackPage } from "@/pages/AuthCallbackPage";
import { ViewerAnalytics } from "@/components/analytics/ViewerAnalytics";
import { isSignedInUser } from "@/lib/authState";

function Shell() {
  return (
    <Layout>
      <ViewerAnalytics />
      <Outlet />
    </Layout>
  );
}

function CreatorRoute() {
  const user = useAppStore((state) => state.user);
  return isSignedInUser(user) ? <Outlet /> : <Navigate to="/" replace />;
}

export const router = createBrowserRouter([
  {
    element: <Shell />,
    children: [
      { path: "/", element: <HomePage /> },
      { path: "/live", element: <BrowseLivePage /> },
      { path: "/live/:slug", element: <LiveWatchPage /> },
      { path: "/series", element: <CatalogPage kind="series" /> },
      { path: "/series/:slug", element: <SeriesPage /> },
      { path: "/films", element: <CatalogPage kind="film" /> },
      { path: "/film/:slug", element: <FilmPage /> },
      { path: "/originals", element: <CatalogPage kind="all" originalsOnly /> },
      { path: "/category/:slug", element: <CategoryPage /> },
      { path: "/search", element: <SearchPage /> },
      { path: "/watch/episode/:id", element: <WatchPage kind="episode" /> },
      { path: "/watch/film/:id", element: <WatchPage kind="film" /> },
      { path: "/watchlist", element: <WatchlistPage /> },
      { path: "/library", element: <LibraryPage /> },
      { path: "/following", element: <FollowingPage /> },
      {
        element: <CreatorRoute />,
        children: [
          { path: "/studio", element: <StudioPage /> },
          { path: "/ad-hub", element: <AdHubPage /> },
        ],
      },
      { path: "/profile", element: <ProfilePage /> },
      { path: "/settings", element: <SettingsPage /> },
      { path: "/auth/callback", element: <AuthCallbackPage /> },
      { path: "/:profileHandle", element: <ProfilePage /> },
      { path: "*", element: <HomePage /> },
    ],
  },
]);
