// QIndex JavaScript Application

document.addEventListener('DOMContentLoaded', function() {
    // Initialize tooltips
    initTooltips();
    
    // Add scroll to top button
    addScrollToTopButton();
    
    // Initialize search autocomplete
    initSearchAutocomplete();
    
    // Add table sorting
    initTableSorting();
    
    // Add loading indicators for AJAX requests
    initAjaxLoading();
});

// Initialize Bootstrap tooltips
function initTooltips() {
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

// Search autocomplete
function initSearchAutocomplete() {
    const searchInput = document.querySelector('input[name="q"]');
    if (!searchInput) return;
    
    let timeout;
    searchInput.addEventListener('input', function() {
        clearTimeout(timeout);
        const query = this.value;
        
        if (query.length < 2) return;
        
        timeout = setTimeout(() => {
            fetch(`/api/search?q=${encodeURIComponent(query)}`)
                .then(response => response.json())
                .then(data => {
                    showSearchSuggestions(data.data, searchInput);
                });
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
    if (data.venues && data.venues.length > 0) {
        const venueHeader = document.createElement('h6');
        venueHeader.className = 'dropdown-header';
        venueHeader.textContent = 'Venues';
        suggestions.appendChild(venueHeader);
        
        data.venues.slice(0, 3).forEach(venue => {
            const item = document.createElement('a');
            item.className = 'dropdown-item';
            item.href = `/venue/${venue.id}`;
            item.innerHTML = `${venue.name} <span class="badge bg-secondary">${venue.tier}</span>`;
            suggestions.appendChild(item);
        });
    }
    
    // Add scholars
    if (data.scholars && data.scholars.length > 0) {
        const scholarHeader = document.createElement('h6');
        scholarHeader.className = 'dropdown-header';
        scholarHeader.textContent = 'Scholars';
        suggestions.appendChild(scholarHeader);
        
        data.scholars.slice(0, 3).forEach(scholar => {
            const item = document.createElement('a');
            item.className = 'dropdown-item';
            item.href = `/scholar/${scholar.id}`;
            item.innerHTML = `${scholar.name} <span class="badge bg-primary">${scholar.qindex.toFixed(1)}</span>`;
            suggestions.appendChild(item);
        });
    }
    
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

// AJAX loading indicators
function initAjaxLoading() {
    let activeRequests = 0;
    
    // Create loading indicator
    const loadingIndicator = document.createElement('div');
    loadingIndicator.id = 'loadingIndicator';
    loadingIndicator.className = 'position-fixed top-0 start-50 translate-middle-x mt-3';
    loadingIndicator.style.display = 'none';
    loadingIndicator.style.zIndex = '9999';
    loadingIndicator.innerHTML = `
        <div class="alert alert-info d-flex align-items-center" role="alert">
            <div class="spinner-border spinner-border-sm me-2" role="status">
                <span class="visually-hidden">Loading...</span>
            </div>
            Loading...
        </div>
    `;
    document.body.appendChild(loadingIndicator);
    
    // Intercept fetch
    const originalFetch = window.fetch;
    window.fetch = function(...args) {
        activeRequests++;
        loadingIndicator.style.display = 'block';
        
        return originalFetch.apply(this, args)
            .finally(() => {
                activeRequests--;
                if (activeRequests === 0) {
                    loadingIndicator.style.display = 'none';
                }
            });
    };
}

// Export data functionality
function exportData(format) {
    const currentUrl = window.location.pathname;
    let exportUrl;
    
    if (currentUrl.includes('venues')) {
        exportUrl = `/api/venues?format=${format}`;
    } else if (currentUrl.includes('scholars')) {
        exportUrl = `/api/scholars?format=${format}`;
    } else {
        exportUrl = `/api/stats?format=${format}`;
    }
    
    window.location.href = exportUrl;
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
Chart.defaults.font.family = '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';
Chart.defaults.color = '#2c3e50';

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
                ${message}
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