.PHONY: build test vet clean dist release

build:
	go build -o tuibik ./cmd/tuibik

test:
	go test -count=1 -race ./...

vet:
	go vet ./...

clean:
	rm -rf dist/ tuibik

# Stubbed — will be filled in PR 5
dist:
	@echo "dist target not yet implemented (see PR 5)"

# Stubbed — will be filled in PR 5
release:
	@echo "release target not yet implemented (see PR 5)"
