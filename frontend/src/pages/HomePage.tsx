import { useEffect, useState } from "react";
import { HeroCarousel } from "@/components/content/HeroCarousel";
import { ContentRow } from "@/components/content/ContentRow";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import type { Film, LiveStream, Series } from "@/types";
import "./HomePage.css";

export function HomePage() {
  const continueWatching = useAppStore((s) => s.continueWatching);
  const [trending, setTrending] = useState<
    ReadonlyArray<Series | Film | LiveStream>
  >([]);
  const [originals, setOriginals] = useState<ReadonlyArray<Series | Film>>([]);
  const [liveNow, setLiveNow] = useState<ReadonlyArray<LiveStream>>([]);
  const [series, setSeries] = useState<ReadonlyArray<Series>>([]);
  const [films, setFilms] = useState<ReadonlyArray<Film>>([]);
  const [sciFi, setSciFi] = useState<ReadonlyArray<Series | Film>>([]);
  const [continueItems, setContinueItems] = useState<
    ReadonlyArray<Series | Film>
  >([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const heroItems = originals.slice(0, 4);

  const progressMap = Object.fromEntries(
    continueWatching.map((c) => [c.contentId, c.progressSec / c.durationSec]),
  );

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);

    void Promise.all([
      repository.fetchHome(controller.signal),
      repository.fetchSeriesPage(
        { originalsOnly: true, limit: 6 },
        controller.signal,
      ),
      repository.fetchFilmsPage(
        { originalsOnly: true, limit: 6 },
        controller.signal,
      ),
      repository.fetchSeriesPage({ limit: 12 }, controller.signal),
      repository.fetchFilmsPage({ limit: 12 }, controller.signal),
      repository.fetchSeriesPage(
        { genre: "Science Fiction", limit: 6 },
        controller.signal,
      ),
      repository.fetchFilmsPage(
        { genre: "Science Fiction", limit: 6 },
        controller.signal,
      ),
      Promise.all(
        continueWatching.map((entry) =>
          repository.fetchContentById(entry.contentId, controller.signal),
        ),
      ),
    ])
      .then(
        ([
          home,
          originalSeries,
          originalFilms,
          seriesPage,
          filmsPage,
          sciFiSeries,
          sciFiFilms,
          continued,
        ]) => {
          setTrending([
            ...home.trendingSeries,
            ...home.trendingFilms,
            ...home.featuredLive,
          ]);
          setOriginals(
            [...originalSeries.items, ...originalFilms.items].slice(0, 10),
          );
          setLiveNow(home.featuredLive);
          setSeries(seriesPage.items);
          setFilms(filmsPage.items);
          setSciFi([...sciFiSeries.items, ...sciFiFilms.items]);
          setContinueItems(
            continued.filter(
              (item): item is Series | Film =>
                item.kind === "series" || item.kind === "film",
            ),
          );
        },
      )
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Unable to load home.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [continueWatching]);

  return (
    <div className="ls-home">
      {loading ? <div className="ls-home__state">Loading home…</div> : null}
      {error ? <div className="ls-home__state">{error}</div> : null}
      <HeroCarousel items={heroItems} />

      <div className="ls-home__rows">
        {continueItems.length > 0 && (
          <ContentRow
            kicker="01 / Your queue"
            title="Continue watching"
            items={continueItems}
            layout="landscape"
            progressById={progressMap}
            seeAllHref="/library"
          />
        )}

        <ContentRow
          kicker="02 / Right now"
          title="Live on VANTA"
          items={liveNow}
          layout="landscape"
          seeAllHref="/live"
        />

        <ContentRow
          kicker="03 / Trending"
          title="Everyone is watching"
          items={trending}
          layout="landscape"
          seeAllHref="/live"
        />

        <ContentRow
          kicker="04 / Originals"
          title="VANTA Originals"
          items={originals}
          layout="landscape"
          seeAllHref="/originals"
        />

        <ContentRow
          kicker="05 / Series"
          title="Series we're obsessed with"
          items={series}
          layout="landscape"
          seeAllHref="/series"
        />

        <ContentRow
          kicker="06 / Films"
          title="Films worth the runtime"
          items={films}
          layout="landscape"
          seeAllHref="/films"
        />

        <ContentRow
          kicker="07 / Genre"
          title="Hard science fiction"
          items={sciFi}
          layout="landscape"
          seeAllHref="/series?genre=Science%20Fiction"
        />
      </div>
    </div>
  );
}
