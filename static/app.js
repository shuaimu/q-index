// QIndex JavaScript Application
//
// The site is static: pages are pre-rendered by `qindex build-site`, and the
// few dynamic bits (filters, search, scholar profiles) run here against JSON
// files under data/.

// URL prefix the site is served under (e.g. "/q-index/"), set by the generator
const BASE = (document.querySelector('meta[name="qindex-base"]') || {}).content || '/';

// Must match SCHOLAR_SHARDS in src/site/mod.rs
const SCHOLAR_SHARDS = 256;
const PAPERS_PER_PAGE = 20;
const MAX_SEARCH_RESULTS = 200;

document.addEventListener('DOMContentLoaded', function() {
    // Initialize tooltips
    initTooltips();

    // Add scroll to top button
    addScrollToTopButton();

    // Initialize search autocomplete
    initSearchAutocomplete();

    // Add table sorting
    initTableSorting();

    // Copy-to-clipboard buttons (e.g. DOIs)
    initCopyButtons();

    // Page-specific behaviour
    const pages = {
        venues: initVenueFilters,
        scholars: initScholarFilters,
        search: renderSearchPage,
        scholar: renderScholarPage,
    };
    const init = pages[document.body.dataset.page];
    if (init) init();
});

// ---------------------------------------------------------------------------
// Helpers

function siteUrl(path) {
    return BASE + path.replace(/^\//, '');
}

function scholarUrl(id) {
    return siteUrl('scholar/') + '?id=' + encodeURIComponent(id);
}

function escapeHtml(value) {
    return String(value)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

function fetchJson(path) {
    return fetch(siteUrl(path)).then(response => {
        if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
        return response.json();
    });
}

// FNV-1a over the UTF-8 bytes of the id. Must match scholar_shard() in
// src/site/mod.rs.
function shardOf(id) {
    let h = 0x811c9dc5;
    for (const b of new TextEncoder().encode(id)) {
        h ^= b;
        h = Math.imul(h, 0x01000193) >>> 0;
    }
    return h % SCHOLAR_SHARDS;
}

let searchIndexPromise = null;

// { venues: [{id, name, full_name, field, tier, pagerank, url}],
//   scholars: [[id, name, qindex, h_index, paper_count], ...] }
function loadSearchIndex() {
    if (!searchIndexPromise) {
        searchIndexPromise = fetchJson('data/search-index.json').catch(error => {
            searchIndexPromise = null;
            throw error;
        });
    }
    return searchIndexPromise;
}

// Same matching as the old server: case-insensitive substring of the venue
// name/full name/field, or of the scholar's name/normalized name.
function searchIndex(index, query) {
    const q = query.trim().toLowerCase();
    if (!q) return { venues: [], scholars: [] };
    const venues = index.venues.filter(v =>
        v.name.toLowerCase().includes(q) ||
        v.full_name.toLowerCase().includes(q) ||
        v.field.toLowerCase().includes(q));
    const scholars = index.scholars
        .filter(([id, name]) =>
            name.toLowerCase().includes(q) || id.replace(/_/g, ' ').includes(q))
        .map(([id, name, qindex, h_index, paper_count]) =>
            ({ id, name, qindex, h_index, paper_count }));
    return { venues, scholars };
}

function errorAlert(message) {
    return `
        <div class="alert alert-danger" role="alert">
            <i class="bi bi-exclamation-triangle me-2"></i>${escapeHtml(message)}
        </div>`;
}

function tierBadge(tier) {
    const cls = tier === 'A*' ? 'bg-danger'
        : tier === 'A' ? 'bg-warning text-dark'
        : tier === 'B' ? 'bg-info text-dark'
        : 'bg-secondary';
    return `<span class="badge ${cls}">${escapeHtml(tier)}</span>`;
}

// Same windowing as the pagination() template in src/site/templates.rs
function paginationHtml(current, total, urlFor) {
    if (total <= 1) return '';
    const item = (cls, inner) => `<li class="${cls}">${inner}</li>`;
    let html = '<nav class="mt-3"><ul class="pagination justify-content-center">';
    html += item(current <= 1 ? 'page-item disabled' : 'page-item',
        `<a class="page-link" href="${current > 1 ? escapeHtml(urlFor(current - 1)) : '#'}">Previous</a>`);
    for (let page = 1; page <= total; page++) {
        if (page === 1 || page === total || (page >= current - 2 && page <= current + 2)) {
            html += item(page === current ? 'page-item active' : 'page-item',
                `<a class="page-link" href="${escapeHtml(urlFor(page))}">${page}</a>`);
        } else if ((page === 2 && current > 4) || (page === total - 1 && current < total - 3)) {
            html += item('page-item disabled', '<span class="page-link">...</span>');
        }
    }
    html += item(current >= total ? 'page-item disabled' : 'page-item',
        `<a class="page-link" href="${current < total ? escapeHtml(urlFor(current + 1)) : '#'}">Next</a>`);
    return html + '</ul></nav>';
}

// ---------------------------------------------------------------------------
// Venues page: ?field= and ?tier= filters

function initVenueFilters() {
    const params = new URLSearchParams(window.location.search);
    const field = params.get('field') || '';
    const tier = params.get('tier') || '';
    document.getElementById('field').value = field;
    document.getElementById('tier').value = tier;

    let rank = 0;
    document.querySelectorAll('#venues-table tbody tr').forEach(row => {
        const show = (!field || row.dataset.field.toLowerCase().includes(field.toLowerCase())) &&
            (!tier || row.dataset.tier === tier);
        row.style.display = show ? '' : 'none';
        if (show) row.querySelector('.rank').textContent = ++rank;
    });
    document.getElementById('venues-empty').classList.toggle('d-none', rank > 0);
}

// ---------------------------------------------------------------------------
// Scholars page: ?min_papers= filter

function initScholarFilters() {
    const minPapers = parseInt(new URLSearchParams(window.location.search).get('min_papers'), 10);
    if (isNaN(minPapers)) return;
    let rank = 0;
    document.querySelectorAll('#scholars-table tbody tr').forEach(row => {
        const show = parseInt(row.dataset.papers, 10) >= minPapers;
        row.style.display = show ? '' : 'none';
        if (show) row.querySelector('.rank').textContent = ++rank;
    });
}

// ---------------------------------------------------------------------------
// Search page

function renderSearchPage() {
    const query = new URLSearchParams(window.location.search).get('q') || '';
    const container = document.getElementById('search-results');
    document.getElementById('search-query').innerHTML =
        `Results for: <strong>${escapeHtml(query)}</strong>`;
    const navInput = document.querySelector('input[name="q"]');
    if (navInput) navInput.value = query;

    loadSearchIndex()
        .then(index => {
            const { venues, scholars } = searchIndex(index, query);
            let html = '';

            if (venues.length > 0) {
                html += `
                    <div class="row mb-4"><div class="col-12">
                        <h3><i class="bi bi-building me-2"></i>Venues</h3>
                        <div class="card"><div class="card-body"><div class="table-responsive">
                        <table class="table table-hover">
                            <thead><tr><th>Venue</th><th>Tier</th><th>Field</th><th>PageRank</th></tr></thead>
                            <tbody>`;
                venues.forEach(venue => {
                    html += `
                        <tr>
                            <td><a href="${escapeHtml(siteUrl(venue.url))}">${escapeHtml(venue.name)}</a></td>
                            <td><span class="badge bg-secondary">${escapeHtml(venue.tier)}</span></td>
                            <td>${escapeHtml(venue.field)}</td>
                            <td>${venue.pagerank.toFixed(4)}</td>
                        </tr>`;
                });
                html += '</tbody></table></div></div></div></div></div>';
            }

            if (scholars.length > 0) {
                const shown = scholars.slice(0, MAX_SEARCH_RESULTS);
                html += `
                    <div class="row mb-4"><div class="col-12">
                        <h3><i class="bi bi-people me-2"></i>Scholars</h3>
                        <div class="card"><div class="card-body">`;
                if (shown.length < scholars.length) {
                    html += `<p class="text-muted">Showing the top ${shown.length} of ${scholars.length} matching scholars. Refine your search to see more.</p>`;
                }
                html += `
                        <div class="table-responsive">
                        <table class="table table-hover">
                            <thead><tr><th>Scholar</th><th>QIndex</th><th>H-Index</th><th>Papers</th></tr></thead>
                            <tbody>`;
                shown.forEach(scholar => {
                    html += `
                        <tr>
                            <td><a href="${escapeHtml(scholarUrl(scholar.id))}">${escapeHtml(scholar.name)}</a></td>
                            <td><span class="badge bg-primary">${scholar.qindex.toFixed(1)}</span></td>
                            <td>${scholar.h_index}</td>
                            <td>${scholar.paper_count}</td>
                        </tr>`;
                });
                html += '</tbody></table></div></div></div></div></div>';
            }

            if (venues.length === 0 && scholars.length === 0) {
                html = `
                    <div class="alert alert-info">
                        <i class="bi bi-info-circle me-2"></i>No results found for your search query.
                    </div>`;
            }
            container.innerHTML = html;
        })
        .catch(error => {
            console.error('Error:', error);
            container.innerHTML = errorAlert('Failed to load the search index.');
        });
}

// ---------------------------------------------------------------------------
// Scholar page: ?id=<scholar id>&page=<n>

function googleScholarUrl(paper) {
    const author = (paper.first_author || '').replace(/ /g, '+');
    return `https://scholar.google.com/scholar?q=${paper.title.replace(/ /g, '+')} ${author}`;
}

function renderScholarPage() {
    const params = new URLSearchParams(window.location.search);
    const id = params.get('id') || '';
    const root = document.getElementById('scholar-root');

    if (!id) {
        root.innerHTML = errorAlert('No scholar selected. Use the search box to find a scholar.');
        return;
    }

    const shard = shardOf(id).toString(16).padStart(2, '0');
    fetchJson(`data/scholars/${shard}.json`)
        .then(scholars => {
            const scholar = scholars[id];
            if (!scholar) {
                root.innerHTML = errorAlert('Scholar not found.');
                return;
            }
            document.title = `${scholar.name} - QIndex`;
            document.getElementById('scholar-crumb').textContent = scholar.name;

            const papers = scholar.papers;
            const totalPages = Math.max(1, Math.ceil(papers.length / PAPERS_PER_PAGE));
            const page = Math.min(Math.max(parseInt(params.get('page'), 10) || 1, 1), totalPages);
            const pagePapers = papers.slice((page - 1) * PAPERS_PER_PAGE, page * PAPERS_PER_PAGE);
            root.innerHTML = scholarHtml(scholar, pagePapers, page, totalPages, id);
        })
        .catch(error => {
            console.error('Error:', error);
            root.innerHTML = errorAlert('Failed to load scholar data.');
        });
}

function scholarHtml(scholar, pagePapers, page, totalPages, id) {
    const affiliations = scholar.affiliations && scholar.affiliations.length
        ? escapeHtml(scholar.affiliations.join(', '))
        : '<span class="text-muted">Unknown</span>';

    const stat = (label, value) => `
        <div class="col-md-3"><div class="stat-box text-center">
            <h5>${label}</h5><div class="display-6">${value}</div>
        </div></div>`;

    let html = `
        <div class="card mb-4 shadow-sm"><div class="card-body">
            <h1 class="card-title mb-3">${escapeHtml(scholar.name)}</h1>
            <div class="row">
                <div class="col-md-3"><div class="stat-box text-center">
                    <h5>Affiliation</h5><p class="lead">${affiliations}</p>
                </div></div>
                ${stat('Papers', scholar.papers.length)}
                ${stat('Citations', scholar.citations)}
                ${stat('H-Index', scholar.h_index)}
            </div>
        </div></div>
        <div class="row">
            <div class="col-md-4 mb-4"><div class="card h-100 shadow-sm">
                <div class="card-header bg-primary text-white"><h5 class="mb-0">Venues</h5></div>
                <div class="card-body">`;
    if (scholar.venues.length === 0) {
        html += '<p class="text-muted">No venues found</p>';
    } else {
        html += '<ul class="list-group list-group-flush">';
        scholar.venues.forEach(([venue, count]) => {
            html += `
                <li class="list-group-item d-flex justify-content-between">
                    ${escapeHtml(venue)}<span class="badge bg-secondary">${count} papers</span>
                </li>`;
        });
        html += '</ul>';
    }
    html += `
                </div>
            </div></div>
            <div class="col-md-8 mb-4"><div class="card h-100 shadow-sm">
                <div class="card-header bg-primary text-white">
                    <div class="d-flex justify-content-between align-items-center">
                        <h5 class="mb-0">Publications (Page ${page} of ${totalPages})</h5>
                        <small>Showing ${pagePapers.length} of ${scholar.papers.length} papers</small>
                    </div>
                </div>
                <div class="card-body">`;
    if (pagePapers.length === 0) {
        html += '<p class="text-muted">No publications found</p>';
    } else {
        html += '<div class="list-group">';
        pagePapers.forEach(paper => { html += paperHtml(paper); });
        html += '</div>';
    }
    html += paginationHtml(page, totalPages, p => `${scholarUrl(id)}&page=${p}`);
    html += '</div></div></div></div>';
    return html;
}

function paperHtml(paper) {
    const citations = paper.s2ag
        ? `Citations: <strong class="text-primary">${paper.citations}</strong> (S2AG)`
        : `Citations: ${paper.citations} (internal)`;
    const publisher = paper.doi ? `https://doi.org/${paper.doi}` : paper.publisher_url;

    let links = `
        <a class="btn btn-outline-primary" href="${escapeHtml(googleScholarUrl(paper))}" target="_blank" title="Search on Google Scholar">
            <i class="bi bi-google me-1"></i>Scholar
        </a>`;
    if (publisher) {
        links += `
            <a class="btn btn-outline-secondary" href="${escapeHtml(publisher)}" target="_blank" title="Publisher Page">
                <i class="bi bi-journal-text me-1"></i>${paper.doi ? 'DOI' : 'Publisher'}
            </a>`;
    }
    if (paper.doi) {
        links += `
            <button class="btn btn-outline-info" type="button" data-copy="${escapeHtml(paper.doi)}" title="Copy DOI to clipboard">
                <i class="bi bi-clipboard"></i>
            </button>`;
    }

    return `
        <div class="list-group-item">
            <h6 class="mb-2">${escapeHtml(paper.title)}</h6>
            <p class="mb-1 text-muted small">
                Venue: ${escapeHtml(paper.venue)}${paper.year ? ` (${paper.year})` : ''}
            </p>
            <div class="d-flex justify-content-between align-items-center mb-2">
                <small class="text-muted">${citations}</small>
            </div>
            <div class="btn-group btn-group-sm">${links}</div>
        </div>`;
}

// ---------------------------------------------------------------------------
// General UI

// Initialize Bootstrap tooltips
function initTooltips() {
    if (typeof bootstrap === 'undefined') return;
    const tooltipTriggerList = [].slice.call(document.querySelectorAll('[data-bs-toggle="tooltip"]'));
    tooltipTriggerList.map(function (tooltipTriggerEl) {
        return new bootstrap.Tooltip(tooltipTriggerEl);
    });
}

// Scroll to top button
function addScrollToTopButton() {
    // Create button
    const scrollBtn = document.createElement('button');
    scrollBtn.id = 'scrollToTop';
    scrollBtn.innerHTML = '<i class="bi bi-arrow-up"></i>';
    document.body.appendChild(scrollBtn);

    // Show/hide based on scroll position
    window.onscroll = function() {
        if (document.body.scrollTop > 100 || document.documentElement.scrollTop > 100) {
            scrollBtn.style.display = 'block';
        } else {
            scrollBtn.style.display = 'none';
        }
    };

    // Scroll to top when clicked
    scrollBtn.onclick = function() {
        window.scrollTo({
            top: 0,
            behavior: 'smooth'
        });
    };
}

// Search autocomplete, backed by the static search index
function initSearchAutocomplete() {
    const searchInput = document.querySelector('input[name="q"]');
    if (!searchInput) return;

    let timeout;
    searchInput.addEventListener('input', function() {
        clearTimeout(timeout);
        const query = this.value;

        if (query.length < 2) return;

        timeout = setTimeout(() => {
            loadSearchIndex()
                .then(index => {
                    // Ignore results for a query the user has since changed
                    if (searchInput.value === query) {
                        showSearchSuggestions(searchIndex(index, query), searchInput);
                    }
                })
                .catch(error => console.error('Error:', error));
        }, 300);
    });
}

function showSearchSuggestions(data, inputElement) {
    // Remove existing suggestions
    const existingSuggestions = document.querySelector('.search-suggestions');
    if (existingSuggestions) {
        existingSuggestions.remove();
    }

    // Create suggestions dropdown
    const suggestions = document.createElement('div');
    suggestions.className = 'search-suggestions dropdown-menu show';
    suggestions.style.position = 'absolute';
    suggestions.style.top = inputElement.offsetHeight + 'px';
    suggestions.style.width = inputElement.offsetWidth + 'px';

    // Add venues
    if (data.venues.length > 0) {
        const venueHeader = document.createElement('h6');
        venueHeader.className = 'dropdown-header';
        venueHeader.textContent = 'Venues';
        suggestions.appendChild(venueHeader);

        data.venues.slice(0, 3).forEach(venue => {
            const item = document.createElement('a');
            item.className = 'dropdown-item';
            item.href = siteUrl(venue.url);
            item.innerHTML = `${escapeHtml(venue.name)} <span class="badge bg-secondary">${escapeHtml(venue.tier)}</span>`;
            suggestions.appendChild(item);
        });
    }

    // Add scholars
    if (data.scholars.length > 0) {
        const scholarHeader = document.createElement('h6');
        scholarHeader.className = 'dropdown-header';
        scholarHeader.textContent = 'Scholars';
        suggestions.appendChild(scholarHeader);

        data.scholars.slice(0, 3).forEach(scholar => {
            const item = document.createElement('a');
            item.className = 'dropdown-item';
            item.href = scholarUrl(scholar.id);
            item.innerHTML = `${escapeHtml(scholar.name)} <span class="badge bg-primary">${scholar.qindex.toFixed(1)}</span>`;
            suggestions.appendChild(item);
        });
    }

    if (!suggestions.children.length) return;

    // Position and add to DOM
    inputElement.parentElement.style.position = 'relative';
    inputElement.parentElement.appendChild(suggestions);

    // Remove on click outside
    document.addEventListener('click', function(e) {
        if (!inputElement.parentElement.contains(e.target)) {
            suggestions.remove();
        }
    });
}

// Table sorting
function initTableSorting() {
    const tables = document.querySelectorAll('table.sortable');

    tables.forEach(table => {
        const headers = table.querySelectorAll('th');

        headers.forEach((header, index) => {
            header.style.cursor = 'pointer';
            header.addEventListener('click', () => {
                sortTable(table, index);
            });
        });
    });
}

function sortTable(table, columnIndex) {
    const tbody = table.querySelector('tbody');
    const rows = Array.from(tbody.querySelectorAll('tr'));

    // Determine sort direction
    const currentOrder = table.dataset.sortOrder || 'asc';
    const newOrder = currentOrder === 'asc' ? 'desc' : 'asc';
    table.dataset.sortOrder = newOrder;

    // Sort rows
    rows.sort((a, b) => {
        const aValue = a.cells[columnIndex].textContent.trim();
        const bValue = b.cells[columnIndex].textContent.trim();

        // Try to parse as number
        const aNum = parseFloat(aValue);
        const bNum = parseFloat(bValue);

        if (!isNaN(aNum) && !isNaN(bNum)) {
            return newOrder === 'asc' ? aNum - bNum : bNum - aNum;
        }

        // Sort as string
        return newOrder === 'asc'
            ? aValue.localeCompare(bValue)
            : bValue.localeCompare(aValue);
    });

    // Reorder rows in table
    rows.forEach(row => tbody.appendChild(row));

    // Update header indicators
    const headers = table.querySelectorAll('th');
    headers.forEach(h => {
        h.classList.remove('sort-asc', 'sort-desc');
    });
    headers[columnIndex].classList.add(`sort-${newOrder}`);
}

// Buttons with data-copy="text" copy that text to the clipboard
function initCopyButtons() {
    document.addEventListener('click', function(e) {
        const button = e.target.closest('[data-copy]');
        if (button) copyToClipboard(button.dataset.copy);
    });
}

// Real-time search
function setupRealtimeSearch() {
    const searchInputs = document.querySelectorAll('.realtime-search');

    searchInputs.forEach(input => {
        input.addEventListener('input', function() {
            const query = this.value.toLowerCase();
            const targetSelector = this.dataset.target;
            const items = document.querySelectorAll(targetSelector);

            items.forEach(item => {
                const text = item.textContent.toLowerCase();
                if (text.includes(query)) {
                    item.style.display = '';
                } else {
                    item.style.display = 'none';
                }
            });
        });
    });
}

// Chart theme
if (typeof Chart !== 'undefined') {
    Chart.defaults.font.family = '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';
    Chart.defaults.color = '#2c3e50';
}

// Utility function to format numbers
function formatNumber(num) {
    return new Intl.NumberFormat('en-US').format(num);
}

// Utility function to copy to clipboard
function copyToClipboard(text) {
    navigator.clipboard.writeText(text).then(() => {
        // Show toast notification
        showToast('Copied to clipboard!');
    });
}

// Toast notification
function showToast(message) {
    const toast = document.createElement('div');
    toast.className = 'position-fixed bottom-0 end-0 p-3';
    toast.style.zIndex = '11';
    toast.innerHTML = `
        <div class="toast show" role="alert">
            <div class="toast-header">
                <strong class="me-auto">QIndex</strong>
                <button type="button" class="btn-close" data-bs-dismiss="toast"></button>
            </div>
            <div class="toast-body">
                ${escapeHtml(message)}
            </div>
        </div>
    `;
    document.body.appendChild(toast);

    setTimeout(() => {
        toast.remove();
    }, 3000);
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', setupRealtimeSearch);
