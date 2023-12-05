%global _cross_first_party 1
%undefine _debugsource_packages

Name: %{_cross_os}os
Version: 0.0
Release: 0%{?dist}
Summary: Temporarily needed to satisfy rpm2migrations: os.spec does this in Bottlerocket
License: Apache-2.0 OR MIT

%description
%{summary}.

%package -n %{_cross_os}migrations
Summary: Thar data store migrations
%description -n %{_cross_os}migrations
%{summary}.

%prep
%setup -T -c
%cargo_prep

%build
mkdir bin

%install

install -d %{buildroot}%{_cross_datadir}/migrations

%files -n %{_cross_os}migrations
%dir %{_cross_datadir}/migrations
%{_cross_datadir}/migrations
