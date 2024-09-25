declare class ComplexWildcardSegment {
    static: string;
}

declare class ComplexSegment {
    static?: string;
    complexWildcard?: ("wildcard" | ComplexWildcardSegment)[];
}

function complexWildcardMatches(items: ("wildcard" | ComplexWildcardSegment)[], segment: string): boolean {
    if (items.length == 0) {
        return segment.length == 0;
    }
    if (items[0] == "wildcard") {
        if (items.length == 1) {
            return true;
        }
        items.shift();
        while (segment.length > 0) {
            if (complexWildcardMatches(items, segment)) {
                return true;
            }
            segment = segment.slice(1);
        }
        return complexWildcardMatches(items, segment);
    } else if (segment.startsWith(items[0].static)) {
        segment = segment.slice(items[0].static.length);
        items.shift();
        return complexWildcardMatches(items, segment);
    } else {
        return false;
    }
}

export class Item {
    segment: "wildcard" | "optionalWildcard" | "repeatedWildcard" | ComplexSegment = "wildcard";
    children: Item[] = [];
    terminating: boolean = false;
    requiresLogin: boolean = false;

    find(path: string): boolean | null {
        let segment: string | null = null;
        let remaining: string | null = null;
        for (let i = 0; i < path.length; i+=1) {
            if (path[i] == "/") {
                segment = path.slice(0, i);
                remaining = path.slice(i+1, path.length);
                break;
            }
        }
        if (segment == null || remaining == null) {
            segment = path;
            remaining = "";
        }

        let matches;
        if (this.segment == "wildcard") {
            matches = true;
        } else if (this.segment == "optionalWildcard") {
            let res = this.findChild(path);
            if (res != null) {
                return res;
            }
            matches = true;
        } else if (this.segment == "repeatedWildcard") {
            let res = this.findChild(path);
            if (res == null && remaining.length > 0) {
                res = this.find(remaining);
            }
            if (res != null) {
                return res;
            }
            matches = true;
        } else {
            let decoded = decodeURIComponent(segment);
            if (this.segment.static != null) {
                matches = this.segment.static == decoded;
            } else if (this.segment.complexWildcard != null) {
                matches = complexWildcardMatches(this.segment.complexWildcard, decoded);
            }
        }

        if (matches) {
            if (remaining.length == 0 && this.terminating) {
                return this.requiresLogin;
            }
            return this.findChild(remaining);
        }
        return null;
    }

    findChild(path: string): boolean | null {
        for (let item of this.children) {
            let res = item.find(path);
            if (res != null) {
                return res;
            }
        }
        return null;
    }

    fromJson(value: any): Item {
        for (let i = 0; i < value.children.length; i += 1) {
            value.children[i] = new Item().fromJson(value.children[i]);
        }
        return Object.assign(this, value);
    }
}

export class SveltePathFinder {
    children: Item[] = [];
    terminating: boolean = false;
    requiresLogin: boolean = false;

    find(path: string): boolean | null {
        if (path[0] == "/") {
            path = path.substring(1);
        }
        
        if (this.terminating && path.length == 0) {
            return this.requiresLogin;
        }
        for (let item of this.children) {
            let res = item.find(path);
            if (res != null) {
                return res;
            }
        }
        return null;
    }

    fromJson(value: any): SveltePathFinder {
        for (let i = 0; i < value.children.length; i += 1) {
            value.children[i] = new Item().fromJson(value.children[i]);
        }
        return Object.assign(this, value);
    }
}
