import { createBrowserRouter, Outlet } from "react-router-dom";
import { Layout } from "@/components/layout/Layout";
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
import { CreatorOverviewPage } from "@/pages/creator/CreatorOverviewPage";
import { CreatorLivePage } from "@/pages/creator/CreatorLivePage";
import { CreatorContentPage } from "@/pages/creator/CreatorContentPage";
import { CreatorAnalyticsPage } from "@/pages/creator/CreatorAnalyticsPage";
import { CreatorRevenuePage } from "@/pages/creator/CreatorRevenuePage";

function Shell() {
  return (
    <Layout>
      <Outlet />
    </Layout>
  );
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
      { path: "/browse", element: <BrowseLivePage /> },
      { path: "/category/:slug", element: <CategoryPage /> },
      { path: "/search", element: <SearchPage /> },
      { path: "/watch/episode/:id", element: <WatchPage kind="episode" /> },
      { path: "/watch/film/:id", element: <WatchPage kind="film" /> },
      { path: "/watchlist", element: <WatchlistPage /> },
      { path: "/library", element: <LibraryPage /> },
      { path: "/following", element: <FollowingPage /> },
      { path: "/profile", element: <ProfilePage /> },
      { path: "/settings", element: <SettingsPage /> },
      { path: "/creator", element: <CreatorOverviewPage /> },
      { path: "/creator/live", element: <CreatorLivePage /> },
      { path: "/creator/content", element: <CreatorContentPage /> },
      { path: "/creator/analytics", element: <CreatorAnalyticsPage /> },
      { path: "/creator/revenue", element: <CreatorRevenuePage /> },
      { path: "*", element: <HomePage /> },
    ],
  },
]);
